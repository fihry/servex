use crate::http::models::method::Method;
use crate::http::models::request::Request;
use crate::http::parser::chunked::{decode_chunked, ChunkedError};
use crate::http::parser::headers::parse_header_lines;

#[derive(Debug)]
pub enum RequestParseError {
    Incomplete,
    Invalid(String),
    Chunked(ChunkedError),
}

pub fn parse_request(buffer: &[u8]) -> Result<(Request, usize), RequestParseError> {
    let header_end = find_header_end(buffer).ok_or(RequestParseError::Incomplete)?;
    let header_block = &buffer[..header_end];
    let header_text = std::str::from_utf8(header_block)
        .map_err(|_| RequestParseError::Invalid("invalid header encoding".to_string()))?;

    let mut lines = header_text.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| RequestParseError::Invalid("missing request line".to_string()))?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts
        .next()
        .ok_or_else(|| RequestParseError::Invalid("missing method".to_string()))?;
    let path = request_parts
        .next()
        .ok_or_else(|| RequestParseError::Invalid("missing path".to_string()))?;
    let version = request_parts
        .next()
        .ok_or_else(|| RequestParseError::Invalid("missing version".to_string()))?;

    let header_lines: Vec<&str> = lines.collect();
    let headers = parse_header_lines(&header_lines)
        .map_err(|_| RequestParseError::Invalid("invalid headers".to_string()))?;

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
            .map_err(|_| RequestParseError::Invalid("invalid content-length".to_string()))?;
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
