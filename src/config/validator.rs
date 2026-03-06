use super::models::*;
use super::server::Server;
pub struct ConfigValidator;

impl ConfigValidator {
    /// Validate entire server configuration
    pub fn validate(config: &ServerConfig) -> Result<(), String> {
        Self::validate_global(&config.global)?;
        Self::validate_error_pages(&config.error_pages)?;
        Self::validate_servers(&config.servers)?;
        Ok(())
    }

    /// Validate global configuration
    fn validate_global(global: &GlobalConfig) -> Result<(), String> {
        if global.max_body_size == 0 {
            return Err("max_body_size must be greater than 0".to_string());
        }

        if global.timeout == 0 {
            return Err("timeout must be greater than 0".to_string());
        }

        Ok(())
    }

    /// Validate error page paths exist
    fn validate_error_pages(
        pages: &std::collections::HashMap<u16, std::path::PathBuf>,
    ) -> Result<(), String> {
        for (code, path) in pages {
            if !path.exists() {
                return Err(format!("Error page for {} not found: {:?}", code, path));
            }

            if !path.is_file() {
                return Err(format!("Error page for {} is not a file: {:?}", code, path));
            }
        }
        Ok(())
    }

    /// Validate all servers
    fn validate_servers(servers: &[Server]) -> Result<(), String> {
        if servers.is_empty() {
            return Err("No server configured".to_string());
        }
        for server in servers {
            Self::validate_server(server)?;
        }
        Ok(())
    }

    /// Validate a single server
    fn validate_server(server: &Server) -> Result<(), String> {
        // Validate host
        if server.host.is_empty() {
            return Err(format!("Server '{}' has empty host", server.name));
        }

        // Validate ports
        if server.ports.is_empty() {
            return Err(format!("Server '{}' has no ports defined", server.name));
        }

        let mut unique_ports = std::collections::HashSet::new();
        for &port in &server.ports {
            if port == 0 {
                return Err(format!("Server '{}' has invalid port 0", server.name));
            }
            if !unique_ports.insert(port) {
                return Err(format!(
                    "Server '{}' has duplicated port {} in its ports list",
                    server.name, port
                ));
            }
        }

        if server.server_names.is_empty() {
            return Err(format!("Server '{}' has no server_name", server.name));
        }

        // Validate root directory exists
        if !server.root.exists() {
            return Err(format!(
                "Server '{}' root directory does not exist: {:?}",
                server.name, server.root
            ));
        }

        if !server.root.is_dir() {
            return Err(format!(
                "Server '{}' root is not a directory: {:?}",
                server.name, server.root
            ));
        }

        // Validate routes
        for route in &server.routes {
            Self::validate_route(route)?;
        }

        Ok(())
    }

    /// Validate a single route
    fn validate_route(route: &Route) -> Result<(), String> {
        // Validate path
        if route.path.is_empty() {
            return Err("Route has empty path".to_string());
        }

        if !route.path.starts_with('/') {
            return Err(format!("Route path must start with '/': {}", route.path));
        }

        // Validate methods
        if route.methods.is_empty() {
            return Err(format!("Route '{}' has no methods defined", route.path));
        }

        for method in &route.methods {
            if !["GET", "POST", "DELETE", "PUT", "HEAD", "OPTIONS"].contains(&method.as_str()) {
                return Err(format!(
                    "Invalid HTTP method '{}' in route '{}'",
                    method, route.path
                ));
            }
        }

        // Validate CGI configuration
        if let Some(cgi) = &route.cgi {
            if !cgi.executor.exists() {
                return Err(format!(
                    "CGI executor not found for route '{}': {:?}",
                    route.path, cgi.executor
                ));
            }

            if cgi.extension.is_empty() {
                return Err(format!("CGI extension is empty for route '{}'", route.path));
            }

            if !cgi.extension.starts_with('.') {
                return Err(format!(
                    "CGI extension must start with '.' for route '{}': {}",
                    route.path, cgi.extension
                ));
            }
        }

        // Validate redirect
        if let Some(redirect) = &route.redirect {
            if ![301, 302, 303, 307, 308].contains(&redirect.status) {
                return Err(format!(
                    "Invalid redirect status {} for route '{}'",
                    redirect.status, route.path
                ));
            }

            if redirect.target.is_empty() {
                return Err(format!(
                    "Redirect target is empty for route '{}'",
                    route.path
                ));
            }
        }

        // Validate upload directory
        if let Some(upload_dir) = &route.upload_dir {
            if !upload_dir.exists() {
                return Err(format!(
                    "Upload directory does not exist for route '{}': {:?}",
                    route.path, upload_dir
                ));
            }

            if !upload_dir.is_dir() {
                return Err(format!(
                    "Upload path is not a directory for route '{}': {:?}",
                    route.path, upload_dir
                ));
            }
        }

        // Validate root if specified
        if let Some(root) = &route.root {
            if !root.exists() {
                return Err(format!(
                    "Route root does not exist for '{}': {:?}",
                    route.path, root
                ));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_path(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("servex_validator_{prefix}_{nanos}"))
    }

    #[test]
    fn test_validate_global_invalid_body_size() {
        let global = GlobalConfig {
            max_body_size: 0,
            timeout: 30,
            keep_alive: true,
        };

        assert!(ConfigValidator::validate_global(&global).is_err());
    }

    #[test]
    fn test_validate_global_invalid_timeout() {
        let global = GlobalConfig {
            max_body_size: 1024,
            timeout: 0,
            keep_alive: true,
        };

        assert!(ConfigValidator::validate_global(&global).is_err());
    }

    #[test]
    fn test_validate_route_invalid_method() {
        let route = Route {
            path: "/test".to_string(),
            methods: vec!["INVALID".to_string()],
            root: None,
            index: None,
            redirect: None,
            cgi: None,
            upload_dir: None,
            autoindex: false,
            max_file_size: None,
        };

        assert!(ConfigValidator::validate_route(&route).is_err());
    }

    #[test]
    fn test_validate_route_path_no_slash() {
        let route = Route {
            path: "test".to_string(),
            methods: vec!["GET".to_string()],
            root: None,
            index: None,
            redirect: None,
            cgi: None,
            upload_dir: None,
            autoindex: false,
            max_file_size: None,
        };

        assert!(ConfigValidator::validate_route(&route).is_err());
    }

    #[test]
    fn test_validate_server_duplicate_ports_rejected() {
        let root = unique_path("dup_ports_root");
        fs::create_dir_all(&root).expect("failed to create root");

        let server = Server {
            name: "main".to_string(),
            server_names: vec!["localhost".to_string()],
            host: "127.0.0.1".to_string(),
            ports: vec![8080, 8080],
            root: root.clone(),
            routes: vec![],
        };

        let err = ConfigValidator::validate_server(&server).expect_err("duplicate ports must fail");
        assert!(err.contains("duplicated port 8080"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn test_validate_server_missing_root_directory() {
        let server = Server {
            name: "main".to_string(),
            server_names: vec!["localhost".to_string()],
            host: "127.0.0.1".to_string(),
            ports: vec![8080],
            root: unique_path("missing_root"),
            routes: vec![],
        };

        let err = ConfigValidator::validate_server(&server).expect_err("missing root should fail");
        assert!(err.contains("root directory does not exist"));
    }

    #[test]
    fn test_validate_route_rejects_cgi_extension_without_dot() {
        let route = Route {
            path: "/cgi".to_string(),
            methods: vec!["GET".to_string()],
            root: None,
            index: None,
            redirect: None,
            cgi: Some(CgiConfig {
                extension: "py".to_string(),
                executor: PathBuf::from("/bin/sh"),
            }),
            upload_dir: None,
            autoindex: false,
            max_file_size: None,
        };

        let err = ConfigValidator::validate_route(&route).expect_err("bad cgi extension must fail");
        assert!(err.contains("must start with '.'"));
    }

    #[test]
    fn test_validate_route_rejects_missing_cgi_executor() {
        let route = Route {
            path: "/cgi".to_string(),
            methods: vec!["GET".to_string()],
            root: None,
            index: None,
            redirect: None,
            cgi: Some(CgiConfig {
                extension: ".py".to_string(),
                executor: unique_path("missing_executor"),
            }),
            upload_dir: None,
            autoindex: false,
            max_file_size: None,
        };

        let err =
            ConfigValidator::validate_route(&route).expect_err("missing cgi executor must fail");
        assert!(err.contains("CGI executor not found"));
    }

    #[test]
    fn test_validate_route_rejects_invalid_redirect_status() {
        let route = Route {
            path: "/old".to_string(),
            methods: vec!["GET".to_string()],
            root: None,
            index: None,
            redirect: Some(Redirect {
                status: 309,
                target: "/new".to_string(),
            }),
            cgi: None,
            upload_dir: None,
            autoindex: false,
            max_file_size: None,
        };

        let err =
            ConfigValidator::validate_route(&route).expect_err("invalid redirect should fail");
        assert!(err.contains("Invalid redirect status 309"));
    }

    #[test]
    fn test_validate_error_pages_rejects_non_file_path() {
        let dir = unique_path("error_pages_dir");
        fs::create_dir_all(&dir).expect("failed to create temp dir");
        let mut pages = HashMap::new();
        pages.insert(404u16, dir.clone());

        let err =
            ConfigValidator::validate_error_pages(&pages).expect_err("directory path should fail");
        assert!(err.contains("is not a file"));
        let _ = fs::remove_dir_all(dir);
    }
}
