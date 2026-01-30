use super::models::*;
use super::parser::IniParser;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct ConfigLoader;

impl ConfigLoader {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<ServerConfig, String> {
        let sections = IniParser::parse_file(path)?;
        Self::build_config(sections)
    }

    fn build_config(sections: HashMap<String, HashMap<String, String>>) -> Result<ServerConfig, String> {
        let mut config = ServerConfig::default();

        // Parse global config
        if let Some(global) = sections.get("global") {
            config.global = Self::parse_global(global)?;
        }

        // Parse error pages
        if let Some(errors) = sections.get("error_pages") {
            config.error_pages = Self::parse_error_pages(errors)?;
        }

        // Parse servers and routes
        let mut servers: HashMap<String, VirtualServer> = HashMap::new();

        for (section_name, section_data) in &sections {
            if let Some(server_name) = section_name.strip_prefix("server:") {
                let server = Self::parse_server(server_name, section_data)?;
                servers.insert(server_name.to_string(), server);
            }
        }

        // Parse routes and add to servers
        for (section_name, section_data) in &sections {
            if let Some(route_path) = section_name.strip_prefix("route:") {
                let parts: Vec<&str> = route_path.splitn(2, ':').collect();
                if parts.len() == 2 {
                    let server_name = parts[0];
                    let route = Self::parse_route(section_data)?;
                    
                    if let Some(server) = servers.get_mut(server_name) {
                        server.routes.push(route);
                    }
                }
            }
        }

        config.servers = servers.into_values().collect();

        Ok(config)
    }

    fn parse_global(data: &HashMap<String, String>) -> Result<GlobalConfig, String> {
        Ok(GlobalConfig {
            max_body_size: data.get("max_body_size")
                .and_then(|s| s.parse().ok())
                .unwrap_or(1_048_576),
            timeout: data.get("timeout")
                .and_then(|s| s.parse().ok())
                .unwrap_or(30),
            keep_alive: data.get("keep_alive")
                .and_then(|s| s.parse().ok())
                .unwrap_or(true),
        })
    }

    fn parse_error_pages(data: &HashMap<String, String>) -> Result<HashMap<u16, PathBuf>, String> {
        let mut pages = HashMap::new();
        for (code, path) in data {
            let code: u16 = code.parse()
                .map_err(|_| format!("Invalid error code: {}", code))?;
            pages.insert(code, PathBuf::from(path));
        }
        Ok(pages)
    }

    fn parse_server(name: &str, data: &HashMap<String, String>) -> Result<VirtualServer, String> {
        let host = data.get("host")
            .ok_or("Server missing 'host'")?
            .to_string();

        let ports: Vec<u16> = data.get("ports")
            .ok_or("Server missing 'ports'")?
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();

        let is_default = data.get("default")
            .and_then(|s| s.parse().ok())
            .unwrap_or(false);

        let root = data.get("root")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("./www"));

        Ok(VirtualServer {
            name: name.to_string(),
            host,
            ports,
            is_default,
            root,
            routes: vec![],
        })
    }

    fn parse_route(data: &HashMap<String, String>) -> Result<Route, String> {
        let path = data.get("path")
            .ok_or("Route missing 'path'")?
            .to_string();

        let methods: Vec<String> = data.get("methods")
            .map(|s| s.split(',').map(|m| m.trim().to_uppercase()).collect())
            .unwrap_or_else(|| vec!["GET".to_string()]);

        let root = data.get("root").map(PathBuf::from);
        let index = data.get("index").map(String::from);
        let autoindex = data.get("autoindex")
            .and_then(|s| s.parse().ok())
            .unwrap_or(false);

        let upload_dir = data.get("upload_dir").map(PathBuf::from);
        let max_file_size = data.get("max_file_size")
            .and_then(|s| s.parse().ok());

        // Parse CGI
        let cgi = if let (Some(ext), Some(exec)) = (
            data.get("cgi_extension"),
            data.get("cgi_executor")
        ) {
            Some(CgiConfig {
                extension: ext.to_string(),
                executor: PathBuf::from(exec),
            })
        } else {
            None
        };

        // Parse redirect
        let redirect = if let (Some(status), Some(target)) = (
            data.get("redirect_status"),
            data.get("redirect_target")
        ) {
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