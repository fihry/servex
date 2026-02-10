use crate::http::models::status::Status;

// Builds Response and converts to bytes
pub struct ResponseBuilder {
   status: Status
}

impl ResponseBuilder {
    pub fn build(&self) -> Vec<u8> {
        let mut response = Vec::new();

        // Status line: "HTTP/1.1 200 OK\r\n"
        response.extend_from_slice(b"HTTP/1.1 ");
        response.extend_from_slice(self.status.code.to_string().as_bytes());
        response.extend_from_slice(b" ");
        response.extend_from_slice(self.status.reason.as_bytes());
        response.extend_from_slice(b"\r\n");

        // Headers...
        // Body...

        response
    }
}