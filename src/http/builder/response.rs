pub struct ResponseBuilder {
    version: String,
    status_line: String,
    content_type: String,
    body: Vec<u8>,
    keep_alive: bool,
    extra_headers: Vec<(String, String)>,
}

impl ResponseBuilder {
    pub fn new(
        version: impl Into<String>,
        status_line: impl Into<String>,
        content_type: impl Into<String>,
        body: Vec<u8>,
        keep_alive: bool,
        extra_headers: Vec<(String, String)>,
    ) -> Self {
        Self {
            version: version.into(),
            status_line: status_line.into(),
            content_type: content_type.into(),
            body,
            keep_alive,
            extra_headers,
        }
    }

    pub fn build(&self) -> Vec<u8> {
        let mut response = Vec::new();
        response.extend_from_slice(self.version.as_bytes());
        response.extend_from_slice(b" ");
        response.extend_from_slice(self.status_line.as_bytes());
        response.extend_from_slice(b"\r\n");

        response.extend_from_slice(b"Content-Type: ");
        response.extend_from_slice(self.content_type.as_bytes());
        response.extend_from_slice(b"\r\n");

        response.extend_from_slice(b"Content-Length: ");
        response.extend_from_slice(self.body.len().to_string().as_bytes());
        response.extend_from_slice(b"\r\n");

        for (key, value) in &self.extra_headers {
            response.extend_from_slice(key.as_bytes());
            response.extend_from_slice(b": ");
            response.extend_from_slice(value.as_bytes());
            response.extend_from_slice(b"\r\n");
        }

        response.extend_from_slice(b"Connection: ");
        response.extend_from_slice(if self.keep_alive { b"keep-alive" } else { b"close" });
        response.extend_from_slice(b"\r\n\r\n");
        response.extend_from_slice(&self.body);
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_sets_connection_close_and_content_length() {
        let response = ResponseBuilder::new(
            "HTTP/1.1",
            "200 OK",
            "text/plain",
            b"hello".to_vec(),
            false,
            Vec::new(),
        )
        .build();
        let text = String::from_utf8(response).expect("response should be utf8");

        assert!(text.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(text.contains("Content-Length: 5\r\n"));
        assert!(text.contains("Connection: close\r\n"));
        assert!(text.ends_with("\r\n\r\nhello"));
    }

    #[test]
    fn build_includes_extra_headers() {
        let response = ResponseBuilder::new(
            "HTTP/1.1",
            "302 Found",
            "text/plain",
            Vec::new(),
            true,
            vec![("Location".to_string(), "/new".to_string())],
        )
        .build();
        let text = String::from_utf8(response).expect("response should be utf8");

        assert!(text.contains("Location: /new\r\n"));
        assert!(text.contains("Connection: keep-alive\r\n"));
    }
}
