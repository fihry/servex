use std::{ collections::HashMap, path::PathBuf };
use super::Route;

#[derive(Debug, Clone)]
pub struct Server {
    pub name: String,
    pub host: String,
    pub ports: Vec<u16>,
    pub root: PathBuf,
    pub routes: Vec<Route>,
}

impl Server {
    pub fn default() -> Self {
        Self {
            name: String::new(),
            host: String::new(),
            ports: vec![],
            root: PathBuf::new(),
            routes: Vec::new(),
        }
    }

    pub fn inject(&mut self, name: &str, data: &HashMap<String, String>) -> Result<(), String> {
        let host = data.get("host").ok_or("Server missing 'host'")?.to_string();

        let ports = match data.get("ports") {
            Some(raw) => {
                let parsed: Vec<u16> = raw
                    .split(',')
                    .filter_map(|part| part.trim().split_whitespace().next())
                    .filter_map(|token| token.parse().ok())
                    .collect();
                if parsed.is_empty() {
                    vec![8080]
                } else {
                    parsed
                }
            }
            None => vec![8080],
        };

        let root = data
            .get("root")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("./www"));

        
        self.name = name.to_string();
        self.host = host;
        self.ports = ports;
        self.root = root;
        Ok(())
    }
}
