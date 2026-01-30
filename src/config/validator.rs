use super::models::*;
use std::path::Path;

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
    fn validate_error_pages(pages: &std::collections::HashMap<u16, std::path::PathBuf>) -> Result<(), String> {
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
    fn validate_servers(servers: &[VirtualServer]) -> Result<(), String> {
        if servers.is_empty() {
            return Err("At least one server must be defined".to_string());
        }

        // Check for port conflicts
        let mut used_ports = std::collections::HashSet::new();
        for server in servers {
            Self::validate_server(server)?;

            for &port in &server.ports {
                if !used_ports.insert((server.host.clone(), port)) {
                    return Err(format!(
                        "Port {} already in use on host {}",
                        port, server.host
                    ));
                }
            }
        }

        // Ensure exactly one default server
        let default_count = servers.iter().filter(|s| s.is_default).count();
        if default_count == 0 {
            return Err("At least one server must be marked as default".to_string());
        }
        if default_count > 1 {
            return Err("Only one server can be marked as default".to_string());
        }

        Ok(())
    }

    /// Validate a single server
    fn validate_server(server: &VirtualServer) -> Result<(), String> {
        // Validate host
        if server.host.is_empty() {
            return Err(format!("Server '{}' has empty host", server.name));
        }

        // Validate ports
        if server.ports.is_empty() {
            return Err(format!("Server '{}' has no ports defined", server.name));
        }

        for &port in &server.ports {
            if port == 0 {
                return Err(format!("Server '{}' has invalid port 0", server.name));
            }
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
                return Err(format!("Invalid HTTP method '{}' in route '{}'", method, route.path));
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
                return Err(format!("Redirect target is empty for route '{}'", route.path));
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
}