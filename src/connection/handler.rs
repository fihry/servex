const HEADER_TERMINATOR: &[u8] = b"\r\n\r\n";

pub fn try_build_response(buffer: &mut Vec<u8>) -> Option<Vec<u8>> {
    if !buffer.windows(HEADER_TERMINATOR.len()).any(|w| w == HEADER_TERMINATOR) {
        return None;
    }

    buffer.clear();
    Some(build_empty_response())
}

fn build_empty_response() -> Vec<u8> {
    let mut response = Vec::new();
    response.extend_from_slice(b"HTTP/1.1 200 OK\r\n");
    response.extend_from_slice(b"Content-Length: 0\r\n");
    response.extend_from_slice(b"Connection: close\r\n");
    response.extend_from_slice(b"\r\n");
    response
}
