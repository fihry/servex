use crate::http::models::headers::Headers;
use crate::http::models::method::Method;

#[derive(Debug, Clone)]
pub struct Request {
    pub method: Method,
    pub path: String,
    pub version: String,
    pub headers: Headers,
    pub body: Vec<u8>,
}

impl Request {
    pub fn new(
        method: Method,
        path: String,
        version: String,
        headers: Headers,
        body: Vec<u8>,
    ) -> Self {
        Self {
            method,
            path,
            version,
            headers,
            body,
        }
    }
}
