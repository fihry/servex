pub mod directory;
pub mod error;
pub mod redirect;
pub mod static_files;
pub mod upload;

#[derive(Debug)]
pub struct HttpResponse {
    pub status_line: String,
    pub content_type: &'static str,
    pub body: Vec<u8>,
    pub extra_headers: Vec<(String, String)>,
}

impl HttpResponse {
    pub fn ok(body: Vec<u8>, content_type: &'static str) -> Self {
        Self {
            status_line: "200 OK".to_string(),
            content_type,
            body,
            extra_headers: Vec::new(),
        }
    }

    pub fn created(body: Vec<u8>, content_type: &'static str) -> Self {
        Self {
            status_line: "201 Created".to_string(),
            content_type,
            body,
            extra_headers: Vec::new(),
        }
    }

    pub fn no_content() -> Self {
        Self {
            status_line: "204 No Content".to_string(),
            content_type: "text/plain; charset=utf-8",
            body: Vec::new(),
            extra_headers: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub enum HandlerError {
    Forbidden,
    NotFound,
    MethodNotAllowed,
    Internal,
}

impl HandlerError {
    pub fn status(&self) -> (u16, &'static str) {
        match self {
            HandlerError::Forbidden => (403, "Forbidden"),
            HandlerError::NotFound => (404, "Not Found"),
            HandlerError::MethodNotAllowed => (405, "Method Not Allowed"),
            HandlerError::Internal => (500, "Internal Server Error"),
        }
    }
}
