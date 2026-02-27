use std::collections::HashMap;
use std::path::PathBuf;
use std::{ fmt, usize };
use super::server::Server;

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub global: GlobalConfig,
    pub server: Server,
    pub error_pages: HashMap<u16, PathBuf>,
    pub sessions: Session,
}

#[derive(Debug, Clone)]
pub struct GlobalConfig {
    pub max_body_size: usize,
    pub timeout: u64,
    pub keep_alive: bool,
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

#[derive(Debug, Clone)]
pub struct Session {
    enabled: bool,
    timeout: usize,
    cookie_name: String,
    secure: bool,
    http_only: bool,
    // same_site: &'a str,
}

impl ServerConfig {
    pub fn default() -> Self {
        Self {
            global: GlobalConfig::default(),
            server: Server::default(),
            error_pages: HashMap::new(),
            sessions: Session::default(),
        }
    }
}

impl Session {
    pub fn default() -> Self {
        Self {
            enabled: false,
            timeout: 0,
            cookie_name: "".to_string(),
            secure: false,
            http_only: false,
        }
    }

    pub fn inject(&mut self, data: &HashMap<String, String>) -> Result<(), String> {
        self.enabled = data
            .get("enabled")
            .ok_or("Missing 'enabled' key in data")?
            .parse::<bool>()
            .map_err(|_| "Value for 'enabled' must be a boolean (true/false)")?;

        self.timeout = data
            .get("timeout")
            .ok_or("Missing 'timeout' key in data")?
            .parse::<usize>()
            .map_err(|_| "Value for 'timeout' must be a positive integer (usize)")?;

        self.cookie_name = data
            .get("cookie_name")
            .ok_or("Missing 'cookie_name' key in data")?
            .to_string(); // String parsing usually doesn't fail, just clones

        self.secure = data
            .get("secure")
            .ok_or("Missing 'secure' key in data")?
            .parse::<bool>()
            .map_err(|_| "Value for 'secure' must be a boolean (true/false)")?;

        self.http_only = data
            .get("http_only")
            .ok_or("Missing 'http_only' key in data")?
            .parse::<bool>()
            .map_err(|_| "Value for 'http_only' must be a boolean (true/false)")?;

        Ok(())
    }
}

impl GlobalConfig {
    pub fn default() -> Self {
        Self {
            max_body_size: 1_048_576, // 1MB
            timeout: 30,
            keep_alive: true,
        }
    }
}

impl fmt::Display for GlobalConfig {
    // This function must return a `fmt::Result`
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Use the write! macro to write the formatted string::new() to the formatter `f`
        // write!(f, "({}, {})", self.x, self.y)
        write!(
            f,
            "max body size: {}\n\ttimeout: {}\n\tkeep alive: {}.\n",
            self.max_body_size,
            self.timeout,
            self.keep_alive
        )
    }
}
impl fmt::Display for Server {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Server: {}", self.name)?;
        writeln!(f, "\thost: {}", self.host)?;
        writeln!(f, "\tports: {:?}", self.ports)?;
        writeln!(f, "\troot: {}", self.root.display())?;

        if !self.routes.is_empty() {
            writeln!(f, "\troutes:")?;
            for route in &self.routes {
                writeln!(f, "\t\t{}", route)?;
            }
        }

        Ok(())
    }
}

impl fmt::Display for Route {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Route: {}", self.path)?;
        writeln!(f, "\tmethods: {:?}", self.methods)?;

        if let Some(root) = &self.root {
            writeln!(f, "\troot: {}", root.display())?;
        }

        if let Some(index) = &self.index {
            writeln!(f, "\tindex: {}", index)?;
        }

        if let Some(redirect) = &self.redirect {
            writeln!(f, "\tredirect: {}", redirect)?;
        }

        if let Some(cgi) = &self.cgi {
            writeln!(f, "\tcgi: {}", cgi)?;
        }

        if let Some(upload_dir) = &self.upload_dir {
            writeln!(f, "\tupload_dir: {}", upload_dir.display())?;
        }

        writeln!(f, "\tautoindex: {}", self.autoindex)?;

        if let Some(size) = self.max_file_size {
            writeln!(f, "\tmax_file_size: {}", size)?;
        }

        Ok(())
    }
}

impl fmt::Display for CgiConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "extension: {}", self.extension)?;
        writeln!(f, "\texecutor: {}", self.executor.display())
    }
}

impl fmt::Display for Redirect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} -> {}", self.status, self.target)
    }
}

impl fmt::Display for ServerConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Global Config:\n\t{}", self.global)?;

        if !self.error_pages.is_empty() {
            writeln!(f, "Error Pages:")?;
            for (code, path) in &self.error_pages {
                writeln!(f, "\t{} => {}", code, path.display())?;
            }
        }
        writeln!(f, "Servers:")?;
        writeln!(f, "\t{}", self.server)?;

        Ok(())
    }
}
