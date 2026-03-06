use crate::http::models::headers::Headers;
use crate::http::parser::headers::parse_header_lines;

#[derive(Debug)]
pub struct MultipartPart {
    pub headers: Headers,
    pub data: Vec<u8>,
}

#[derive(Debug)]
pub enum MultipartError {
    InvalidBoundary,
    InvalidHeaders,
}

pub fn parse_multipart(body: &[u8], boundary: &str) -> Result<Vec<MultipartPart>, MultipartError> {
    if boundary.is_empty() {
        return Err(MultipartError::InvalidBoundary);
    }

    let mut parts = Vec::new();
    let boundary_marker = format!("--{}", boundary);
    let boundary_bytes = boundary_marker.as_bytes();

    let mut cursor = 0;
    while cursor < body.len() {
        let boundary_pos = match find_boundary(body, boundary_bytes, cursor) {
            Some(pos) => pos,
            None => break,
        };
        let start = boundary_pos + boundary_bytes.len();
        if body.len() >= start + 2 && &body[start..start + 2] == b"--" {
            break;
        }
        let part_start = skip_crlf(body, start);
        let header_end = find_header_end(body, part_start).ok_or(MultipartError::InvalidHeaders)?;
        let header_block = &body[part_start..header_end];
        let header_text = String::from_utf8_lossy(header_block);
        let header_lines: Vec<&str> = header_text.split("\r\n").collect();
        let headers =
            parse_header_lines(&header_lines).map_err(|_| MultipartError::InvalidHeaders)?;

        let data_start = header_end + 4;
        let next_boundary = find_boundary(body, boundary_bytes, data_start)
            .ok_or(MultipartError::InvalidBoundary)?;
        let mut data_end = next_boundary;
        if data_end >= 2 && &body[data_end - 2..data_end] == b"\r\n" {
            data_end -= 2;
        }
        parts.push(MultipartPart {
            headers,
            data: body[data_start..data_end].to_vec(),
        });

        cursor = next_boundary;
    }

    Ok(parts)
}

fn find_boundary(body: &[u8], boundary: &[u8], start: usize) -> Option<usize> {
    body[start..]
        .windows(boundary.len())
        .position(|window| window == boundary)
        .map(|pos| start + pos)
}

fn find_header_end(body: &[u8], start: usize) -> Option<usize> {
    body[start..]
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|pos| start + pos)
}

fn skip_crlf(body: &[u8], mut index: usize) -> usize {
    if body.len() >= index + 2 && &body[index..index + 2] == b"\r\n" {
        index += 2;
    }
    index
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_boundary() {
        let err = parse_multipart(b"", "").expect_err("empty boundary should fail");
        assert!(matches!(err, MultipartError::InvalidBoundary));
    }

    #[test]
    fn rejects_part_with_invalid_headers() {
        let body = b"--abc\r\nbroken-header\r\n\r\ncontent\r\n--abc--\r\n";
        let err = parse_multipart(body, "abc").expect_err("invalid headers should fail");
        assert!(matches!(err, MultipartError::InvalidHeaders));
    }

    #[test]
    fn parses_multiple_parts() {
        let body = b"--abc\r\nContent-Disposition: form-data; name=\"a\"\r\n\r\none\r\n--abc\r\nContent-Disposition: form-data; name=\"b\"\r\n\r\ntwo\r\n--abc--\r\n";
        let parts = parse_multipart(body, "abc").expect("multipart should parse");

        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].data, b"one");
        assert_eq!(parts[1].data, b"two");
        assert!(parts[0].headers.get("content-disposition").is_some());
        assert!(parts[1].headers.get("content-disposition").is_some());
    }
}
