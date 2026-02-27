use std::path::PathBuf;

use crate::config::{server::Server, models::{Route, ServerConfig}};
use crate::http::models::method::Method;

#[derive(Clone, Debug)]
pub enum RouteDecision {
    NotFound,
    MethodNotAllowed,
    Redirect { status: u16, target: String },
    Matched {
        route_path: String,
        request_path: String,
        method: Method,
        root: PathBuf,
        index: Option<String>,
        autoindex: bool,
        upload_dir: Option<PathBuf>,
    },
}

#[derive(Clone)]
pub struct Router {
    default_server: Server,
}

impl Router {
    pub fn new(config: &ServerConfig) -> Result<Self, String> {
        let default_server = config
            .server.clone();
        Ok(Self { default_server })
    }

    pub fn resolve(&self, request_path: &str, method: &Method) -> RouteDecision {
        let route = self.best_route(request_path);
        let route = match route {
            Some(route) => route,
            None => return RouteDecision::NotFound,
        };

        if let Some(redirect) = &route.redirect {
            return RouteDecision::Redirect {
                status: redirect.status,
                target: redirect.target.clone(),
            };
        }

        if !is_method_allowed(route, method) {
            return RouteDecision::MethodNotAllowed;
        }

        let root = route
            .root
            .clone()
            .unwrap_or_else(|| self.default_server.root.clone());

        RouteDecision::Matched {
            route_path: route.path.clone(),
            request_path: request_path.to_string(),
            method: method.clone(),
            root,
            index: route.index.clone(),
            autoindex: route.autoindex,
            upload_dir: route.upload_dir.clone(),
        }
    }

    fn best_route(&self, request_path: &str) -> Option<&Route> {
        self.default_server
            .routes
            .iter()
            .filter(|route| path_matches(&route.path, request_path))
            .max_by_key(|route| route.path.len())
    }
}

fn path_matches(route_path: &str, request_path: &str) -> bool {
    if route_path == "/" {
        return request_path.starts_with('/');
    }

    request_path == route_path || request_path.starts_with(&format!("{}/", route_path))
}

fn is_method_allowed(route: &Route, method: &Method) -> bool {
    let method_str = method.to_string();
    route.methods.iter().any(|allowed| allowed == &method_str)
}
