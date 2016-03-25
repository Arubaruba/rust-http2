use std::collections::HashMap;
use std::io::Read;
use std::io;
use std::str;
use std::u8;

const BUFFER_SIZE: usize = 8192; // 8 KB - the maximum header size

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

#[derive(Eq, PartialEq, Debug)]
pub enum ParserState<T> {
    /// The parser does not yet have what it needs in the buffer
    FillingBuffer,
    Done(T),
}

/// Waits until initial line and headers have been received then parses everything.
/// It does not handle requests with a body. It simply stops after the headers are done.
pub struct RequestParser {
    buffer_position: usize,
    buffer: [u8; BUFFER_SIZE],
}

impl RequestParser {
    pub fn new() -> Self {
        RequestParser {
            buffer_position: 0,
            buffer: [0; BUFFER_SIZE],
        }
    }

    pub fn read_stream<'a, T: Read>(&'a mut self,
                                    mut stream: T)
                                    -> Result<ParserState<Request<'a>>, ParserError> {

        // We borrow a part of the buffer inside this block so it will go out of scope and whole buffer will be available
        let done = {
            // Borrow the remaining empty section of the buffer as a slice
            let mut remaining_buffer = &mut self.buffer[self.buffer_position..];

            let chars_read = try!(stream.read(remaining_buffer)
                                        .map_err(|err| ParserError::IOError(err)));

            self.buffer_position += chars_read;

            let additional_content = try!(str::from_utf8(&remaining_buffer[..chars_read])
                                              .map_err(|err| ParserError::Uft8Error(err)));

            // Does the newly parsed content contain an empty line? That means all headers have been sent.
            additional_content.contains("\r\n\r\n") || additional_content.contains("\n\n")
        };

        if done {
            let complete_buffer = try!(str::from_utf8(&self.buffer[..self.buffer_position])
                                           .map_err(|err| ParserError::Uft8Error(err)));

            let request = try!(RequestParser::parse(&complete_buffer));
            Ok(ParserState::Done(request))
        } else {
            // The empty line is not there so the server is still sending headers
            Ok(ParserState::FillingBuffer)
        }
    }

    fn parse<'a>(content: &'a str) -> Result<Request<'a>, ParserError> {
        use self::ParserError::InvalidFormat;

        let mut lines = content.lines();

        let mut initial_line = try!(lines.next().ok_or(InvalidFormat { line: 1 }))
                                   .split_whitespace();

        let method = try!(initial_line.next().ok_or(InvalidFormat { line: 1 }));
        let url = try!(initial_line.next().ok_or(InvalidFormat { line: 1 }));
        let version_text = try!(initial_line.next().ok_or(InvalidFormat { line: 1 }));
        let version = try!(RequestParser::parse_version(version_text));

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
	use std::io;
    use std::io::{Read, Write};

    const RAW_REQUEST: &'static str = "GET 	/test/1234  HTTP/1.1\n  Header1 : it\n  Header2:   \
                                       works  \n\n";

    struct MockStream<'a> {
        request_chunks: [&'a [u8]; 2],
        chunk_index: usize,
    }

    impl<'a> MockStream<'a> {
        fn new(mock_request_text: &'a str) -> MockStream<'a> {
            let (request_chunk1, request_chunk2) = mock_request_text.as_bytes().split_at(15);

            MockStream {
                request_chunks: [request_chunk1, request_chunk2],
                chunk_index: 0,
            }
        }
    }

    impl<'a> Read for MockStream<'a> {
        fn read(&mut self, mut buffer: &mut [u8]) -> io::Result<usize> {
            let byte_count = self.request_chunks[self.chunk_index].len();
            buffer.write(self.request_chunks[self.chunk_index]).unwrap();
            self.chunk_index += 1;

            Ok(byte_count)
        }
    }

    #[test]
    fn request_parser_buffering() {
        let mut mock_stream = MockStream::new(RAW_REQUEST);

        let mut parser = RequestParser::new();

        assert_eq!(parser.read_stream(&mut mock_stream).unwrap(),
                   ParserState::FillingBuffer);

        if let ParserState::Done(request) = parser.read_stream(&mut mock_stream).unwrap() {
            assert_eq!(request.url, "/test/1234");
        } else {
            panic!();
        }
    }

    #[test]
    fn parse_request() {

        let request = RequestParser::parse(&RAW_REQUEST).unwrap();
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
        let valid_version = RequestParser::parse_version("HTTP/1.0").unwrap();
        let invalid_version1 = RequestParser::parse_version("HTTP_1.0");
        let invalid_version2 = RequestParser::parse_version("HTTP/1");

        assert_eq!(valid_version,
                   HttpVersion {
                       major: 1,
                       minor: 0,
                   });
        assert!(invalid_version1.is_err());
        assert!(invalid_version2.is_err());
    }
}
