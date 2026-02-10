use crate::http::models::headers::Headers;
use super::status::Status;

pub struct Response {
    pub status: Status,
    pub headers: Headers,
    pub body: Vec<u8>,
}

impl Response {
    pub fn new(status: Status) -> Self{
        Response {
            status,
            headers: Headers::new(),
            body: Vec::new(),
        }
    }
}