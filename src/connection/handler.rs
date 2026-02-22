use crate::http::parser::request::{parse_request, RequestParseError};

pub fn try_build_response(buffer: &mut Vec<u8>) -> Option<Vec<u8>> {
    match parse_request(buffer) {
        Ok((request, consumed)) => {
            buffer.drain(..consumed);
            let body = format!(
                "method={}\npath={}\nversion={}\nbody_len={}\n",
                request.method,
                request.path,
                request.version,
                request.body.len()
            );
            Some(build_response("200 OK", "text/plain; charset=utf-8", body.as_bytes()))
        }
        Err(RequestParseError::Incomplete) => None,
        Err(_) => {
            buffer.clear();
            Some(build_response("400 Bad Request", "text/plain; charset=utf-8", b"Bad Request\n"))
        }
    }
}

fn build_response(status: &str, content_type: &str, body: &[u8]) -> Vec<u8> {
    let mut response = Vec::new();
    response.extend_from_slice(b"HTTP/1.1 ");
    response.extend_from_slice(status.as_bytes());
    response.extend_from_slice(b"\r\n");
    response.extend_from_slice(b"Content-Type: ");
    response.extend_from_slice(content_type.as_bytes());
    response.extend_from_slice(b"\r\n");
    response.extend_from_slice(b"Content-Length: ");
    response.extend_from_slice(body.len().to_string().as_bytes());
    response.extend_from_slice(b"\r\n");
    response.extend_from_slice(b"Connection: close\r\n");
    response.extend_from_slice(b"\r\n");
    response.extend_from_slice(body);
    response
}
