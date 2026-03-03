use crate::http::models::headers::Headers;

#[derive(Debug)]
pub enum HeaderParseError {
    InvalidLine(()),
}

pub fn parse_header_lines(lines: &[&str]) -> Result<Headers, HeaderParseError> {
    let mut headers = Headers::new();

    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let (key, value) = line
            .split_once(':')
            .ok_or_else(|| HeaderParseError::InvalidLine(()))?;
        headers.insert(key.trim(), value.trim());
    }

    Ok(headers)
}
