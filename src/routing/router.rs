use std::path::PathBuf;

use crate::config::{
    models::{CgiConfig, Route, ServerConfig},
    server::Server,
};
use crate::http::models::method::Method;

#[derive(Clone, Debug)]
#[allow(dead_code)]

pub enum RouteDecision {
    NotFound,
    MethodNotAllowed,
    Redirect {
        status: u16,
        target: String,
    },
    Matched {
        route_path: String,
        request_path: String,
        method: Method,
        root: PathBuf,
        index: Option<String>,
        autoindex: bool,
        upload_dir: Option<PathBuf>,
        cgi: Option<CgiConfig>,
        max_file_size: Option<usize>,
    },
}

#[derive(Clone)]
pub struct Router {
    servers: Vec<Server>,
}

impl Router {
    pub fn new(config: &ServerConfig) -> Result<Self, String> {
        Ok(Self {
            servers: config.servers.clone(),
        })
    }

    pub fn select_server<'a>(
        &'a self,
        candidate_servers: &[usize],
        host_header: Option<&str>,
    ) -> Option<&'a Server> {
        if candidate_servers.is_empty() {
            return None;
        }

        if let Some(host) = host_header.and_then(normalized_host_from_header) {
            for index in candidate_servers {
                if let Some(server) = self.servers.get(*index) {
                    if server
                        .server_names
                        .iter()
                        .any(|name| name.eq_ignore_ascii_case(host))
                    {
                        return Some(server);
                    }
                }
            }
        }

        self.servers.get(candidate_servers[0])
    }

    pub fn resolve(&self, server: &Server, request_path: &str, method: &Method) -> RouteDecision {
        let route = best_route(server, request_path);
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
            .unwrap_or_else(|| server.root.clone());

        RouteDecision::Matched {
            route_path: route.path.clone(),
            request_path: request_path.to_string(),
            method: method.clone(),
            root,
            index: route.index.clone(),
            autoindex: route.autoindex,
            upload_dir: route.upload_dir.clone(),
            cgi: route.cgi.clone(),
            max_file_size: route.max_file_size,
        }
    }
}

fn best_route<'a>(server: &'a Server, request_path: &str) -> Option<&'a Route> {
    server
        .routes
        .iter()
        .filter(|route| path_matches(&route.path, request_path))
        .max_by_key(|route| route.path.len())
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

fn normalized_host_from_header(raw_host: &str) -> Option<&str> {
    let host = raw_host.trim();
    if host.is_empty() {
        return None;
    }
    if let Some((value, _)) = host.rsplit_once(':') {
        if !value.is_empty() && !value.contains(']') && host.matches(':').count() == 1 {
            return Some(value);
        }
    }
    Some(host)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::models::{Redirect, Route};
    use crate::config::server::Server;

    fn make_route(path: &str, methods: &[&str]) -> Route {
        Route {
            path: path.to_string(),
            methods: methods.iter().map(|m| (*m).to_string()).collect(),
            root: None,
            index: None,
            redirect: None,
            cgi: None,
            upload_dir: None,
            autoindex: false,
            max_file_size: None,
        }
    }

    fn make_server(name: &str, server_names: &[&str], routes: Vec<Route>) -> Server {
        Server {
            name: name.to_string(),
            server_names: server_names.iter().map(|v| (*v).to_string()).collect(),
            host: "127.0.0.1".to_string(),
            ports: vec![8080],
            root: PathBuf::from("/tmp"),
            routes,
        }
    }

    #[test]
    fn select_server_uses_case_insensitive_host_with_port() {
        let router = Router {
            servers: vec![
                make_server("one", &["example.com"], vec![]),
                make_server("two", &["api.local"], vec![]),
            ],
        };

        let selected = router
            .select_server(&[0, 1], Some("API.LOCAL:8080"))
            .expect("server should be selected");
        assert_eq!(selected.name, "two");
    }

    #[test]
    fn select_server_falls_back_to_first_candidate() {
        let router = Router {
            servers: vec![
                make_server("first", &["first.local"], vec![]),
                make_server("second", &["second.local"], vec![]),
            ],
        };

        let selected = router
            .select_server(&[1, 0], Some("unknown.local"))
            .expect("fallback server should be selected");
        assert_eq!(selected.name, "second");
    }

    #[test]
    fn resolve_uses_longest_matching_prefix_route() {
        let short = make_route("/api", &["GET"]);
        let mut long = make_route("/api/v1", &["GET"]);
        long.autoindex = true;
        let server = make_server("s", &["localhost"], vec![short, long]);
        let router = Router {
            servers: vec![server.clone()],
        };

        let decision = router.resolve(&server, "/api/v1/users", &Method::Get);
        match decision {
            RouteDecision::Matched { route_path, autoindex, .. } => {
                assert_eq!(route_path, "/api/v1");
                assert!(autoindex);
            }
            other => panic!("expected matched route, got {:?}", other),
        }
    }

    #[test]
    fn resolve_returns_method_not_allowed_when_method_not_in_route() {
        let route = make_route("/upload", &["POST"]);
        let server = make_server("s", &["localhost"], vec![route]);
        let router = Router {
            servers: vec![server.clone()],
        };

        let decision = router.resolve(&server, "/upload", &Method::Get);
        assert!(matches!(decision, RouteDecision::MethodNotAllowed));
    }

    #[test]
    fn resolve_returns_redirect_when_route_has_redirect() {
        let mut route = make_route("/old", &["GET"]);
        route.redirect = Some(Redirect {
            status: 301,
            target: "/new".to_string(),
        });
        let server = make_server("s", &["localhost"], vec![route]);
        let router = Router {
            servers: vec![server.clone()],
        };

        let decision = router.resolve(&server, "/old", &Method::Get);
        match decision {
            RouteDecision::Redirect { status, target } => {
                assert_eq!(status, 301);
                assert_eq!(target, "/new");
            }
            other => panic!("expected redirect, got {:?}", other),
        }
    }

    #[test]
    fn resolve_returns_not_found_when_no_route_matches() {
        let server = make_server("s", &["localhost"], vec![make_route("/ok", &["GET"])]);
        let router = Router {
            servers: vec![server.clone()],
        };

        let decision = router.resolve(&server, "/missing", &Method::Get);
        assert!(matches!(decision, RouteDecision::NotFound));
    }
}
