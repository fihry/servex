use super::models::*;
use super::parser::IniParser;
use super::server::Server;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct ConfigLoader;

impl ConfigLoader {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<ServerConfig, String> {
        let sections = IniParser::parse_file(path)?;
        Self::build_config(sections)
    }

    fn build_config(
        sections: HashMap<String, HashMap<String, String>>
    ) -> Result<ServerConfig, String> {
        let mut config: ServerConfig = ServerConfig::default();
        // Parse global config
        if let Some(global) = sections.get("global") {
            config.global = Self::parse_global(global)?;
        }

        // Parse error pages
        if let Some(errors) = sections.get("error_pages") {
            config.error_pages = Self::parse_error_pages(errors)?;
        }

        for (section_name, section_data) in &sections {
            if section_name == "session" {
                config.sessions.inject(section_data)?;
                continue;
            }

            if section_name == "server" {
                let mut server = Server::default();
                server.inject("default", section_data)?;
                config.servers.push(server);
                continue;
            }

            if let Some(server_name) = section_name.strip_prefix("server:") {
                let mut server = Server::default();
                server.inject(server_name, section_data)?;
                config.servers.push(server);
            }
        }

        if config.servers.is_empty() {
            return Err("No server sections found".to_string());
        }

        for (section_name, section_data) in &sections {
            if let Some(route_path) = section_name.strip_prefix("route:") {
                let (server_name, _) = match route_path.split_once(':') {
                    Some(parts) => parts,
                    None => return Err(format!("Invalid route section '{}'", section_name)),
                };
                let route = Self::parse_route(section_data)?;
                if let Some(server) = config
                    .servers
                    .iter_mut()
                    .find(|server| server.name == server_name)
                {
                    server.routes.push(route);
                } else if config.servers.len() == 1 {
                    config.servers[0].routes.push(route);
                } else {
                    return Err(format!(
                        "Route section '{}' references unknown server '{}'",
                        section_name, server_name
                    ));
                }
            } else if section_name.starts_with("route:") {
                if section_name == "route:" {
                    return Err(format!("Invalid route section '{}'", section_name));
                }
            }
        }

        Ok(config)
    }

    fn parse_global(data: &HashMap<String, String>) -> Result<GlobalConfig, String> {
        Ok(GlobalConfig {
            max_body_size: data
                .get("max_body_size")
                .and_then(|s| s.parse().ok())
                .unwrap_or(1_048_576),
            timeout: data
                .get("timeout")
                .and_then(|s| s.parse().ok())
                .unwrap_or(30),
            keep_alive: data
                .get("keep_alive")
                .and_then(|s| s.parse().ok())
                .unwrap_or(true),
        })
    }

    fn parse_error_pages(data: &HashMap<String, String>) -> Result<HashMap<u16, PathBuf>, String> {
        let mut pages = HashMap::new();
        for (code, path) in data {
            let code: u16 = code.parse().map_err(|_| format!("Invalid error code: {}", code))?;
            pages.insert(code, PathBuf::from(path));
        }
        Ok(pages)
    }

    fn parse_route(data: &HashMap<String, String>) -> Result<Route, String> {
        let path = data.get("path").ok_or("Route missing 'path'")?.to_string();

        let methods: Vec<String> = data
            .get("methods")
            .map(|s|
                s
                    .split(',')
                    .map(|m| m.trim().to_uppercase())
                    .collect()
            )
            .unwrap_or_else(|| vec!["GET".to_string()]);

        let root = data.get("root").map(PathBuf::from);
        let index = data.get("index").map(String::from);
        let autoindex = data
            .get("autoindex")
            .and_then(|s| s.parse().ok())
            .unwrap_or(false);

        let upload_dir = data.get("upload_dir").map(PathBuf::from);
        let max_file_size = data.get("max_file_size").and_then(|s| s.parse().ok());

        // Parse CGI
        let cgi = if
            let (Some(ext), Some(exec)) = (data.get("cgi_extension"), data.get("cgi_executor"))
        {
            Some(CgiConfig {
                extension: ext.to_string(),
                executor: PathBuf::from(exec),
            })
        } else {
            None
        };

        // Parse redirect
        let redirect = if
            let (Some(status), Some(target)) = (
                data.get("redirect_status"),
                data.get("redirect_target"),
            )
        {
            Some(Redirect {
                status: status.parse().map_err(|_| "Invalid redirect status")?,
                target: target.to_string(),
            })
        } else {
            None
        };

        Ok(Route {
            path,
            methods,
            root,
            index,
            redirect,
            cgi,
            upload_dir,
            autoindex,
            max_file_size,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kv(values: &[(&str, &str)]) -> HashMap<String, String> {
        let mut out = HashMap::new();
        for (k, v) in values {
            out.insert((*k).to_string(), (*v).to_string());
        }
        out
    }

    #[test]
    fn build_config_errors_when_no_server_sections_exist() {
        let mut sections = HashMap::new();
        sections.insert("global".to_string(), kv(&[("timeout", "10")]));

        let err = ConfigLoader::build_config(sections).expect_err("expected missing server error");
        assert_eq!(err, "No server sections found");
    }

    #[test]
    fn build_config_rejects_invalid_route_section_format() {
        let mut sections = HashMap::new();
        sections.insert(
            "server:main".to_string(),
            kv(&[
                ("host", "127.0.0.1"),
                ("server_name", "localhost"),
                ("root", "./www"),
            ]),
        );
        sections.insert(
            "route:main".to_string(),
            kv(&[("path", "/x"), ("methods", "GET")]),
        );

        let err =
            ConfigLoader::build_config(sections).expect_err("invalid route section should fail");
        assert_eq!(err, "Invalid route section 'route:main'");
    }

    #[test]
    fn build_config_rejects_route_referencing_unknown_server_when_many_servers() {
        let mut sections = HashMap::new();
        sections.insert(
            "server:one".to_string(),
            kv(&[
                ("host", "127.0.0.1"),
                ("server_name", "one.local"),
                ("root", "./www"),
            ]),
        );
        sections.insert(
            "server:two".to_string(),
            kv(&[
                ("host", "127.0.0.1"),
                ("server_name", "two.local"),
                ("root", "./www"),
            ]),
        );
        sections.insert(
            "route:ghost:home".to_string(),
            kv(&[("path", "/"), ("methods", "GET")]),
        );

        let err = ConfigLoader::build_config(sections)
            .expect_err("unknown server route reference should fail");
        assert_eq!(
            err,
            "Route section 'route:ghost:home' references unknown server 'ghost'"
        );
    }

    #[test]
    fn parse_error_pages_rejects_non_numeric_code() {
        let pages = kv(&[("abc", "error_pages/404.html")]);
        let err = ConfigLoader::parse_error_pages(&pages).expect_err("invalid code should fail");
        assert_eq!(err, "Invalid error code: abc");
    }

    #[test]
    fn parse_route_applies_defaults_when_optional_fields_missing() {
        let route_data = kv(&[("path", "/ok")]);
        let route = ConfigLoader::parse_route(&route_data).expect("route should parse");

        assert_eq!(route.path, "/ok");
        assert_eq!(route.methods, vec!["GET".to_string()]);
        assert_eq!(route.index, None);
        assert_eq!(route.autoindex, false);
        assert!(route.redirect.is_none());
    }

    #[test]
    fn parse_route_parses_redirect_and_cgi() {
        let route_data = kv(&[
            ("path", "/legacy"),
            ("methods", "GET,POST"),
            ("redirect_status", "302"),
            ("redirect_target", "/new"),
            ("cgi_extension", ".py"),
            ("cgi_executor", "/usr/bin/python3"),
        ]);
        let route = ConfigLoader::parse_route(&route_data).expect("route should parse");

        assert_eq!(route.methods, vec!["GET".to_string(), "POST".to_string()]);
        assert_eq!(route.redirect.as_ref().map(|r| r.status), Some(302));
        assert_eq!(
            route.redirect.as_ref().map(|r| r.target.as_str()),
            Some("/new")
        );
        assert_eq!(
            route.cgi.as_ref().map(|cgi| cgi.extension.as_str()),
            Some(".py")
        );
    }

    #[test]
    fn parse_route_rejects_invalid_redirect_status() {
        let route_data = kv(&[
            ("path", "/legacy"),
            ("redirect_status", "abc"),
            ("redirect_target", "/new"),
        ]);

        let err = ConfigLoader::parse_route(&route_data).expect_err("bad redirect should fail");
        assert_eq!(err, "Invalid redirect status");
    }

    #[test]
    fn build_config_attaches_route_to_single_server_even_with_name_mismatch() {
        let mut sections = HashMap::new();
        sections.insert(
            "server:main".to_string(),
            kv(&[
                ("host", "127.0.0.1"),
                ("server_name", "localhost"),
                ("root", "./www"),
            ]),
        );
        sections.insert(
            "route:ghost:ok".to_string(),
            kv(&[("path", "/ok"), ("methods", "GET")]),
        );

        let config = ConfigLoader::build_config(sections).expect("single-server fallback should work");
        assert_eq!(config.servers.len(), 1);
        assert_eq!(config.servers[0].routes.len(), 1);
        assert_eq!(config.servers[0].routes[0].path, "/ok");
    }
}
