use std::sync::Arc;

use crate::app::AppContext;
use crate::handlers::error::load_error_page;
use crate::handlers::redirect::handle_redirect;
use crate::handlers::static_files::handle_get;
use crate::handlers::upload::{handle_delete, handle_post};
use crate::handlers::{HandlerError, HttpResponse};
use crate::http::builder::response::ResponseBuilder;
use crate::http::models::method::Method;
use crate::http::parser::request::{parse_request, RequestParseError};
use crate::routing::RouteDecision;

pub struct ParsedResponse {
    pub bytes: Vec<u8>,
    pub close_after_write: bool,
}

#[derive(Clone)]
pub struct RequestHandler {
    context: Arc<AppContext>,
}

impl RequestHandler {
    pub fn new(context: Arc<AppContext>) -> Self {
        Self { context }
    }

    pub fn try_build_response(&self, buffer: &mut Vec<u8>) -> Option<ParsedResponse> {
        match parse_request(buffer) {
            Ok((request, consumed)) => {
                buffer.drain(..consumed);
                let keep_alive =
                    should_keep_alive(&request.version, request.headers.get("connection"));
                if request.body.len() > self.context.max_body_size {
                    return Some(self.error_response(
                        &request.version,
                        413,
                        "Payload Too Large",
                        keep_alive,
                    ));
                }
                Some(self.dispatch_request(
                    &request.version,
                    &request.path,
                    &request.method,
                    request.headers.get("content-type"),
                    &request.body,
                    keep_alive,
                ))
            }
            Err(RequestParseError::Incomplete) => None,
            Err(_) => {
                buffer.clear();
                Some(self.error_response("HTTP/1.1", 400, "Bad Request", false))
            }
        }
    }

    fn dispatch_request(
        &self,
        version: &str,
        request_path: &str,
        method: &Method,
        content_type: Option<&str>,
        body: &[u8],
        keep_alive: bool,
    ) -> ParsedResponse {
        if is_forbidden_path(request_path) {
            return self.error_response(version, 403, "Forbidden", keep_alive);
        }

        match self.context.router.resolve(request_path, method) {
            RouteDecision::NotFound => self.error_response(version, 404, "Not Found", keep_alive),
            RouteDecision::MethodNotAllowed => {
                self.error_response(version, 405, "Method Not Allowed", keep_alive)
            }
            RouteDecision::Redirect { status, target } => {
                self.success_response(version, keep_alive, Ok(handle_redirect(status, target)))
            }
            RouteDecision::Matched {
                route_path,
                request_path,
                method,
                root,
                index,
                autoindex,
                upload_dir,
            } => {
                let result = match method {
                    Method::Get => {
                        handle_get(&root, &route_path, &request_path, index.as_deref(), autoindex)
                    }
                    Method::Post => {
                        handle_post(
                            &root,
                            &route_path,
                            &request_path,
                            upload_dir.as_deref(),
                            content_type,
                            body,
                        )
                    }
                    Method::Delete => {
                        handle_delete(&root, &route_path, &request_path, upload_dir.as_deref())
                    }
                    _ => Err(HandlerError::MethodNotAllowed),
                };
                self.success_response(version, keep_alive, result)
            }
        }
    }

    fn success_response(
        &self,
        version: &str,
        keep_alive: bool,
        result: Result<HttpResponse, HandlerError>,
    ) -> ParsedResponse {
        match result {
            Ok(ok) => ParsedResponse {
                bytes: build_response(version, ok, keep_alive),
                close_after_write: !keep_alive,
            },
            Err(error) => {
                let (code, reason) = error.status();
                self.error_response(version, code, reason, keep_alive)
            }
        }
    }

    fn error_response(&self, version: &str, code: u16, reason: &str, keep_alive: bool) -> ParsedResponse {
        let body = load_error_page(&self.context.error_pages, code, reason).into_bytes();
        let response = HttpResponse {
            status_line: format!("{} {}", code, reason),
            content_type: "text/html; charset=utf-8",
            body,
            extra_headers: Vec::new(),
        };

        ParsedResponse {
            bytes: build_response(version, response, keep_alive),
            close_after_write: !keep_alive,
        }
    }
}

fn build_response(version: &str, response: HttpResponse, keep_alive: bool) -> Vec<u8> {
    let body = response.body;
    let status_line = response.status_line;
    let content_type = response.content_type.to_string();
    let extra_headers = response.extra_headers;
    ResponseBuilder::new(
        version,
        status_line,
        content_type,
        body,
        keep_alive,
        extra_headers,
    )
    .build()
}

fn should_keep_alive(version: &str, connection_header: Option<&str>) -> bool {
    let connection = connection_header.unwrap_or("").trim().to_ascii_lowercase();
    match version {
        "HTTP/1.1" => connection != "close",
        "HTTP/1.0" => connection == "keep-alive",
        _ => false,
    }
}

fn is_forbidden_path(path: &str) -> bool {
    path.contains("..") || path.to_ascii_lowercase().contains("%2e%2e")
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Arc;

    use crate::app::AppContext;
    use crate::config::models::{GlobalConfig, Route, ServerConfig, VirtualServer};

    use super::{should_keep_alive, RequestHandler};

    fn test_handler() -> RequestHandler {
        let config = ServerConfig {
            global: GlobalConfig {
                max_body_size: 1024,
                timeout: 30,
                keep_alive: true,
            },
            error_pages: HashMap::new(),
            servers: vec![VirtualServer {
                name: "main".to_string(),
                host: "127.0.0.1".to_string(),
                ports: vec![8080],
                is_default: true,
                root: PathBuf::from("."),
                routes: vec![Route {
                    path: "/".to_string(),
                    methods: vec!["GET".to_string(), "POST".to_string(), "DELETE".to_string()],
                    root: Some(PathBuf::from(".")),
                    index: None,
                    redirect: None,
                    cgi: None,
                    upload_dir: Some(PathBuf::from("./uploads")),
                    autoindex: false,
                    max_file_size: None,
                }],
            }],
        };

        let context = Arc::new(AppContext::new(&config).expect("context should build"));
        RequestHandler::new(context)
    }

    #[test]
    fn http11_defaults_to_keep_alive() {
        assert!(should_keep_alive("HTTP/1.1", None));
    }

    #[test]
    fn http11_close_header_disables_keep_alive() {
        assert!(!should_keep_alive("HTTP/1.1", Some("close")));
    }

    #[test]
    fn rejects_unsupported_method_with_405() {
        let handler = test_handler();
        let mut raw = b"PUT /x HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n".to_vec();
        let response = handler
            .try_build_response(&mut raw)
            .expect("response expected");
        let text = String::from_utf8(response.bytes).expect("valid utf8 response");
        assert!(text.starts_with("HTTP/1.1 405 Method Not Allowed\r\n"));
    }

    #[test]
    fn returns_413_for_large_payload() {
        let handler = test_handler();
        let body = "a".repeat(2048);
        let mut raw = format!(
            "POST /upload HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        )
        .into_bytes();
        let response = handler
            .try_build_response(&mut raw)
            .expect("response expected");
        let text = String::from_utf8(response.bytes).expect("valid utf8 response");
        assert!(text.starts_with("HTTP/1.1 413 Payload Too Large\r\n"));
    }

    #[test]
    fn parses_two_pipelined_requests_from_same_buffer() {
        let handler = test_handler();
        let mut raw = b"GET /one HTTP/1.1\r\nHost: localhost\r\n\r\nGET /two HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n".to_vec();
        let first = handler
            .try_build_response(&mut raw)
            .expect("first response should parse");
        assert!(!first.close_after_write);
        let second = handler
            .try_build_response(&mut raw)
            .expect("second response should parse");
        assert!(second.close_after_write);
    }
}
