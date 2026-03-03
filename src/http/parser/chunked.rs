#[derive(Debug)]
pub enum ChunkedError {
    InvalidSize,
    InvalidFormat,
    Incomplete,
}

pub fn decode_chunked(body: &[u8]) -> Result<(Vec<u8>, usize), ChunkedError> {
    let mut output = Vec::new();
    let mut index = 0;

    loop {
        let line_end = find_crlf(body, index).ok_or(ChunkedError::Incomplete)?;
        let size_line = std::str::from_utf8(&body[index..line_end]).map_err(|_| ChunkedError::InvalidSize)?;
        let size_part = size_line.split(';').next().unwrap_or(size_line);
        let size = usize::from_str_radix(size_part.trim(), 16).map_err(|_| ChunkedError::InvalidSize)?;
        index = line_end + 2;

        if size == 0 {
            let trailer_end = find_crlf(body, index).ok_or(ChunkedError::Incomplete)?;
            index = trailer_end + 2;
            return Ok((output, index));
        }

        if body.len() < index + size + 2 {
            return Err(ChunkedError::Incomplete);
        }

        output.extend_from_slice(&body[index..index + size]);
        index += size;

        if &body[index..index + 2] != b"\r\n" {
            return Err(ChunkedError::InvalidFormat);
        }
        index += 2;
    }
}

fn find_crlf(buffer: &[u8], start: usize) -> Option<usize> {
    buffer[start..]
        .windows(2)
        .position(|window| window == b"\r\n")
        .map(|pos| start + pos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_chunk_size() {
        let body = b"Z\r\nhello\r\n0\r\n\r\n";
        let err = decode_chunked(body).expect_err("chunk size should be invalid");
        assert!(matches!(err, ChunkedError::InvalidSize));
    }

    #[test]
    fn rejects_invalid_chunk_format_without_trailing_crlf() {
        let body = b"5\r\nhelloX0\r\n\r\n";
        let err = decode_chunked(body).expect_err("format should be invalid");
        assert!(matches!(err, ChunkedError::InvalidFormat));
    }

    #[test]
    fn reports_incomplete_when_last_chunk_terminator_missing() {
        let body = b"5\r\nhello\r\n0\r\n";
        let err = decode_chunked(body).expect_err("body should be incomplete");
        assert!(matches!(err, ChunkedError::Incomplete));
    }
}
