use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub global: GlobalConfig,
    pub servers: Vec<VirtualServer>,
    pub error_pages: HashMap<u16, PathBuf>,
}

#[derive(Debug, Clone)]
pub struct GlobalConfig {
    pub max_body_size: usize,
    pub timeout: u64,
    pub keep_alive: bool,
}

#[derive(Debug, Clone)]
pub struct VirtualServer {
    pub name: String,
    pub host: String,
    pub ports: Vec<u16>,
    pub is_default: bool,
    pub root: PathBuf,
    pub routes: Vec<Route>,
}

#[derive(Debug, Clone)]
pub struct Route {
    pub path: String,
    pub methods: Vec<String>,
    pub root: Option<PathBuf>,
    pub index: Option<String>,
    pub redirect: Option<Redirect>,
    pub cgi: Option<CgiConfig>,
    pub upload_dir: Option<PathBuf>,
    pub autoindex: bool,
    pub max_file_size: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct CgiConfig {
    pub extension: String,
    pub executor: PathBuf,
}

#[derive(Debug, Clone)]
pub struct Redirect {
    pub status: u16,
    pub target: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            global: GlobalConfig::default(),
            servers: vec![],
            error_pages: HashMap::new(),
        }
    }
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            max_body_size: 1_048_576, // 1MB
            timeout: 30,
            keep_alive: true,
        }
    }
}