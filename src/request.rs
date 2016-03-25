use std::collections::HashMap;
use std::io;
use std::str;
use std::u8;

#[derive(Eq, PartialEq, Debug)]
pub struct HttpVersion {
    major: u8,
    minor: u8,
}

#[derive(Eq, PartialEq, Debug)]
pub struct Request<'a> {
    pub method: &'a str,
    pub url: &'a str,
    pub version: HttpVersion,

    // Header names are lowercased so we need a String to modify them
    pub headers: HashMap<String, &'a str>,
}

#[derive(Debug)]
pub enum ParserError {
    IOError(io::Error),
    Uft8Error(str::Utf8Error),
    InvalidFormat {
        line: usize,
    },
    InvalidHttpVersion,
}

impl<'b> Request<'b> {
    pub fn from_str<'a>(text: &'a str) -> Result<Request<'a>, ParserError> {
        use self::ParserError::InvalidFormat;

        let mut lines = text.lines();

        let mut initial_line = try!(lines.next().ok_or(InvalidFormat { line: 1 }))
                                   .split_whitespace();

        let method = try!(initial_line.next().ok_or(InvalidFormat { line: 1 }));
        let url = try!(initial_line.next().ok_or(InvalidFormat { line: 1 }));
        let version_text = try!(initial_line.next().ok_or(InvalidFormat { line: 1 }));
        let version = try!(Request::parse_version(version_text));

        let mut headers = HashMap::new();

        // Parse headers
        for (line_number, header_line) in lines.enumerate() {
			// The last line is purposefully empty
            if header_line.len() > 0 {
                let mut header_parts = header_line.split(":");
                // Add 2 to the line number because line number zero should be labeled as line 2 (counting the initial line)
                let name = try!(header_parts.next().ok_or(InvalidFormat { line: line_number + 2 }))
                               .trim();
                let value = try!(header_parts.next()
                                             .ok_or(InvalidFormat { line: line_number + 2 }))
                                .trim_left();


                // Header names should always be lowercased
                headers.insert(name.to_lowercase(), value);
            } else {
            	break;
            }
        }

        Ok(Request {
            method: method,
            url: url,
            version: version,
            headers: headers,
        })
    }

    fn parse_version<'a>(text: &'a str) -> Result<HttpVersion, ParserError> {
        use self::ParserError::InvalidHttpVersion;

        let mut digits = try!(text.split("/")
                                  .nth(1)
                                  .map(|parts| parts.split("."))
                                  .ok_or(InvalidHttpVersion));

        let mut parse_next_digit = || {
            digits.next().map_or(Err(InvalidHttpVersion), |digit| {
                u8::from_str_radix(digit, 10).map_err(|_| InvalidHttpVersion)
            })
        };

        Ok(HttpVersion {
            major: try!(parse_next_digit()),
            minor: try!(parse_next_digit()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RAW_REQUEST: &'static str = "GET 	/test/1234  HTTP/1.1\n  Header1 : it\n  Header2:   \
                                       works  \n\n";

    #[test]
    fn parse_request() {

        let request = Request::from_str(&RAW_REQUEST).unwrap();
        // The initial line
        assert_eq!(request.method, "GET");
        assert_eq!(request.url, "/test/1234");
        assert_eq!(request.version,
                   HttpVersion {
                       major: 1,
                       minor: 1,
                   });

        // Parser should handle spaces correctly and also make all headers lowercased
        assert_eq!(request.headers.get("header1"), Some(&"it"));
        // Note that spaces at the end of header values are preserved
        assert_eq!(request.headers.get("header2"), Some(&"works  "));
    }

    #[test]
    fn parse_version() {
        let valid_version = Request::parse_version("HTTP/1.0").unwrap();
        let invalid_version1 = Request::parse_version("HTTP_1.0");
        let invalid_version2 = Request::parse_version("HTTP/1");

        assert_eq!(valid_version,
                   HttpVersion {
                       major: 1,
                       minor: 0,
                   });
        assert!(invalid_version1.is_err());
        assert!(invalid_version2.is_err());
    }
}
