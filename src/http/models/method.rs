use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Delete,
    Put,
    Head,
    Options,
    Patch,
    Other(String),
}

impl Method {
    pub fn from_str(value: &str) -> Self {
        match value {
            "GET" => Self::Get,
            "POST" => Self::Post,
            "DELETE" => Self::Delete,
            "PUT" => Self::Put,
            "HEAD" => Self::Head,
            "OPTIONS" => Self::Options,
            "PATCH" => Self::Patch,
            other => Self::Other(other.to_string()),
        }
    }
}

impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Method::Get => write!(f, "GET"),
            Method::Post => write!(f, "POST"),
            Method::Delete => write!(f, "DELETE"),
            Method::Put => write!(f, "PUT"),
            Method::Head => write!(f, "HEAD"),
            Method::Options => write!(f, "OPTIONS"),
            Method::Patch => write!(f, "PATCH"),
            Method::Other(value) => write!(f, "{}", value),
        }
    }
}
