use crate::http::models::method::Method;
use crate::http::models::request::Request;
use crate::http::parser::chunked::{decode_chunked, ChunkedError};
use crate::http::parser::headers::parse_header_lines;

#[derive(Debug)]
pub enum RequestParseError {
    Incomplete,
    Invalid(()),
    Chunked(ChunkedError),
}

pub fn parse_request(buffer: &[u8]) -> Result<(Request, usize), RequestParseError> {
    let header_end = find_header_end(buffer).ok_or(RequestParseError::Incomplete)?;
    let header_block = &buffer[..header_end];
    let header_text = std::str::from_utf8(header_block)
        .map_err(|_| RequestParseError::Invalid(()))?;

    let mut lines = header_text.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| RequestParseError::Invalid(()))?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts
        .next()
        .ok_or_else(|| RequestParseError::Invalid(()))?;
    let path = request_parts
        .next()
        .ok_or_else(|| RequestParseError::Invalid(()))?;
    let version = request_parts
        .next()
        .ok_or_else(|| RequestParseError::Invalid(()))?;

    let header_lines: Vec<&str> = lines.collect();
    let headers = parse_header_lines(&header_lines)
        .map_err(|_| RequestParseError::Invalid(()))?;

    let body_start = header_end + 4;
    let mut consumed = body_start;
    let mut body = Vec::new();

    if let Some(encoding) = headers.get("transfer-encoding") {
        if encoding.eq_ignore_ascii_case("chunked") {
            let (decoded, used) = decode_chunked(&buffer[body_start..])
                .map_err(RequestParseError::Chunked)?;
            body = decoded;
            consumed = body_start + used;
        }
    } else if let Some(length) = headers.get("content-length") {
        let length_value: usize = length
            .trim()
            .parse()
            .map_err(|_| RequestParseError::Invalid(()))?;
        if buffer.len() < body_start + length_value {
            return Err(RequestParseError::Incomplete);
        }
        body.extend_from_slice(&buffer[body_start..body_start + length_value]);
        consumed = body_start + length_value;
    }

    Ok((
        Request::new(
            Method::from_str(method),
            path.to_string(),
            version.to_string(),
            headers,
            body,
        ),
        consumed,
    ))
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_get_request_without_body() {
        let raw = b"GET /hello HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let (request, consumed) = parse_request(raw).expect("request should parse");

        assert_eq!(request.path, "/hello");
        assert_eq!(request.version, "HTTP/1.1");
        assert_eq!(request.body, Vec::<u8>::new());
        assert_eq!(consumed, raw.len());
    }

    #[test]
    fn parses_content_length_body() {
        let raw = b"POST /upload HTTP/1.1\r\nHost: localhost\r\nContent-Length: 5\r\n\r\nhello";
        let (request, consumed) = parse_request(raw).expect("request should parse");

        assert_eq!(request.path, "/upload");
        assert_eq!(request.body, b"hello");
        assert_eq!(consumed, raw.len());
    }

    #[test]
    fn parses_chunked_body() {
        let raw = b"POST /chunk HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n0\r\n\r\n";
        let (request, consumed) = parse_request(raw).expect("chunked request should parse");

        assert_eq!(request.body, b"hello");
        assert_eq!(consumed, raw.len());
    }

    #[test]
    fn rejects_malformed_headers() {
        let raw = b"GET / HTTP/1.1\r\nHost localhost\r\n\r\n";
        let error = parse_request(raw).expect_err("request should fail");
        assert!(matches!(error, RequestParseError::Invalid(_)));
    }

    #[test]
    fn reports_incomplete_body() {
        let raw = b"POST /x HTTP/1.1\r\nContent-Length: 10\r\n\r\nabc";
        let error = parse_request(raw).expect_err("request should be incomplete");
        assert!(matches!(error, RequestParseError::Incomplete));
    }
}
