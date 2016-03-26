use std::collections::HashMap;
use std::{str, u8};

#[derive(Eq, PartialEq, Debug)]
pub struct HttpVersion {
    major: u8,
    minor: u8,
}

#[derive(Eq, PartialEq, Debug)]
pub struct Request<'a> {
    pub method: Method,
    pub url: &'a str,
    pub version: HttpVersion,

    // Header names are lowercased so we need a String to modify them
    pub headers: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub enum ParserError {
    InvalidFormat,
    InvalidHeader(String),
    InvalidHttpVersion,
    InvalidInitialLine(String),
    Uft8Error(str::Utf8Error),
}

#[derive(Eq, PartialEq, Debug)]
pub enum Method {
    DELETE,
    GET,
    POST,
    PUT,
    UPDATE,
    UNSUPPORTED(String),
}

impl From<str::Utf8Error> for ParserError {
    fn from(err: str::Utf8Error) -> Self {
        ParserError::Uft8Error(err)
    }
}

impl<'b> Request<'b> {
    pub fn from_str<'a>(request_text: &'a str) -> Result<Request<'a>, ParserError> {
        use self::ParserError::*;

        // Parse the initial line
        let mut split_at_initial_line = request_text.splitn(2, '\n');
        let initial_line = try!(split_at_initial_line.next()
                                                     .ok_or(InvalidInitialLine(String::new())));

        let (method, url, version) = try!(match initial_line.split_whitespace()
                                                            .collect::<Vec<_>>()
                                                            .as_slice() {
            [method, url, version] => {
                use self::Method::*;

                let method = match method {
                    "DELETE" => DELETE,
                    "GET" => GET,
                    "POST" => POST,
                    "PUT" => PUT,
                    "UPDATE" => UPDATE,
                    _ => UNSUPPORTED(method.to_string()),
                };

                Ok((method, url, try!(Request::parse_version(version))))
            }
            _ => Err(InvalidInitialLine(initial_line.to_string())),
        });

        let remaining_request = try!(split_at_initial_line.next().ok_or(InvalidFormat));

        let empty_line = if initial_line.ends_with('\r') {
            "\r\n\r\n"
        } else {
            "\n\n"
        };

        let mut split_at_empty_line = remaining_request.splitn(2, empty_line);

        let header_text = try!(split_at_empty_line.next().ok_or(InvalidFormat));

        Ok(Request {
            method: method,
            url: url,
            version: version,
            headers: try!(Request::parse_headers(header_text)),
        })
    }

    fn parse_headers<'a>(header_text: &'a str) -> Result<HashMap<String, String>, ParserError> {
        use self::ParserError::InvalidHeader;

        let mut header_lines = header_text.lines().peekable();

        let mut headers = HashMap::<String, String>::new();

        while let Some(line) = header_lines.next() {
            // If this line in a continuation of another header value ignore it
            if line.trim_left().len() == line.len() {
                let err = InvalidHeader(line.to_string());

                let mut parts = line.splitn(2, ':');
                let name = try!(parts.next().ok_or(err.clone())).trim_right().to_lowercase();
                let value = try!(parts.next().ok_or(err.clone())).trim_left();

                let value_continuation = if let Some(next_header) = header_lines.peek() {
                    // If the next header begins with whitespace it should be
                    // interpreted as a continuation of the previous header's value
                    if next_header.trim_left().len() != next_header.len() {
                        Some(next_header.trim_left())
                    } else {
                        None
                    }
                } else {
                    None
                };

                if let Some(value_continuation) = value_continuation {
                    headers.insert(name, value.to_string() + " " + value_continuation);
                } else {
                    headers.insert(name, value.to_string());
                }
            }
        }

        Ok(headers)
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

    const RAW_REQUEST: &'static str = "GET 	/test/1234  HTTP/1.1\nHeader1 : it\nHeader2:   \
                                       works  \n\n";

    #[test]
    fn parse_request() {

        let request = Request::from_str(RAW_REQUEST).unwrap();
        // The initial line
        assert_eq!(request.method, Method::GET);
        assert_eq!(request.url, "/test/1234");
        assert_eq!(request.version,
                   HttpVersion {
                       major: 1,
                       minor: 1,
                   });

        // Parser should handle spaces correctly and also make all headers lowercased
        assert_eq!(request.headers.get("header1"), Some(&"it".to_string()));
        // Note that spaces at the end of header values are preserved
        assert_eq!(request.headers.get("header2"), Some(&"works  ".to_string()));
    }

    #[test]
    fn parser_headers() {
        let header_text = "Header1: 1234\nHeader2 : the\n	 fox jumped";
        let headers = Request::parse_headers(header_text).unwrap();

        assert_eq!(headers.get("header1"), Some(&"1234".to_string()));
        assert_eq!(headers.get("header2"), Some(&"the fox jumped".to_string()));
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
