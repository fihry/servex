// src/http/models/status.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Status {
    pub code: u16,
    pub reason: &'static str,
}

#[allow(dead_code)]
impl Status {
    // Informational 1xx
    pub const CONTINUE: Status = Status {
        code: 100,
        reason: "Continue",
    };
    pub const SWITCHING_PROTOCOLS: Status = Status {
        code: 101,
        reason: "Switching Protocols",
    };

    // Success 2xx
    pub const OK: Status = Status {
        code: 200,
        reason: "OK",
    };
    pub const CREATED: Status = Status {
        code: 201,
        reason: "Created",
    };
    pub const ACCEPTED: Status = Status {
        code: 202,
        reason: "Accepted",
    };
    pub const NO_CONTENT: Status = Status {
        code: 204,
        reason: "No Content",
    };

    // Redirection 3xx
    pub const MOVED_PERMANENTLY: Status = Status {
        code: 301,
        reason: "Moved Permanently",
    };
    pub const FOUND: Status = Status {
        code: 302,
        reason: "Found",
    };
    pub const SEE_OTHER: Status = Status {
        code: 303,
        reason: "See Other",
    };
    pub const NOT_MODIFIED: Status = Status {
        code: 304,
        reason: "Not Modified",
    };
    pub const TEMPORARY_REDIRECT: Status = Status {
        code: 307,
        reason: "Temporary Redirect",
    };
    pub const PERMANENT_REDIRECT: Status = Status {
        code: 308,
        reason: "Permanent Redirect",
    };

    // Client Errors 4xx
    pub const BAD_REQUEST: Status = Status {
        code: 400,
        reason: "Bad Request",
    };
    pub const UNAUTHORIZED: Status = Status {
        code: 401,
        reason: "Unauthorized",
    };
    pub const FORBIDDEN: Status = Status {
        code: 403,
        reason: "Forbidden",
    };
    pub const NOT_FOUND: Status = Status {
        code: 404,
        reason: "Not Found",
    };
    pub const METHOD_NOT_ALLOWED: Status = Status {
        code: 405,
        reason: "Method Not Allowed",
    };
    pub const REQUEST_TIMEOUT: Status = Status {
        code: 408,
        reason: "Request Timeout",
    };
    pub const PAYLOAD_TOO_LARGE: Status = Status {
        code: 413,
        reason: "Payload Too Large",
    };

    // Server Errors 5xx
    pub const INTERNAL_SERVER_ERROR: Status = Status {
        code: 500,
        reason: "Internal Server Error",
    };
    pub const NOT_IMPLEMENTED: Status = Status {
        code: 501,
        reason: "Not Implemented",
    };
    pub const BAD_GATEWAY: Status = Status {
        code: 502,
        reason: "Bad Gateway",
    };
    pub const SERVICE_UNAVAILABLE: Status = Status {
        code: 503,
        reason: "Service Unavailable",
    };
    pub const GATEWAY_TIMEOUT: Status = Status {
        code: 504,
        reason: "Gateway Timeout",
    };
    pub const HTTP_VERSION_NOT_SUPPORTED: Status = Status {
        code: 505,
        reason: "HTTP Version Not Supported",
    };

    // Helper methods
    pub fn from_code(code: u16) -> Option<Status> {
        match code {
            // Informational 1xx
            100 => Some(Status::CONTINUE),
            101 => Some(Status::SWITCHING_PROTOCOLS),

            // Success 2xx
            200 => Some(Status::OK),
            201 => Some(Status::CREATED),
            202 => Some(Status::ACCEPTED),
            204 => Some(Status::NO_CONTENT),

            // Redirection 3xx
            301 => Some(Status::MOVED_PERMANENTLY),
            302 => Some(Status::FOUND),
            303 => Some(Status::SEE_OTHER),
            304 => Some(Status::NOT_MODIFIED),
            307 => Some(Status::TEMPORARY_REDIRECT),
            308 => Some(Status::PERMANENT_REDIRECT),

            // Client Errors 4xx
            400 => Some(Status::BAD_REQUEST),
            401 => Some(Status::UNAUTHORIZED),
            403 => Some(Status::FORBIDDEN),
            404 => Some(Status::NOT_FOUND),
            405 => Some(Status::METHOD_NOT_ALLOWED),
            408 => Some(Status::REQUEST_TIMEOUT),
            413 => Some(Status::PAYLOAD_TOO_LARGE),

            // Server Errors 5xx
            500 => Some(Status::INTERNAL_SERVER_ERROR),
            501 => Some(Status::NOT_IMPLEMENTED),
            502 => Some(Status::BAD_GATEWAY),
            503 => Some(Status::SERVICE_UNAVAILABLE),
            504 => Some(Status::GATEWAY_TIMEOUT),
            505 => Some(Status::HTTP_VERSION_NOT_SUPPORTED),

            _ => None,
        }
    }

    pub fn is_informational(&self) -> bool {
        (100..200).contains(&self.code)
    }

    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.code)
    }

    pub fn is_redirection(&self) -> bool {
        (300..400).contains(&self.code)
    }

    pub fn is_client_error(&self) -> bool {
        (400..500).contains(&self.code)
    }

    pub fn is_server_error(&self) -> bool {
        (500..600).contains(&self.code)
    }
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.code, self.reason)
    }
}
