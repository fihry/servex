use std::collections::HashMap;
use std::path::PathBuf;

use crate::config::models::ServerConfig;
use crate::routing::Router;

#[derive(Clone)]
pub struct AppContext {
    pub router: Router,
    pub max_body_size: usize,
    pub error_pages: HashMap<u16, PathBuf>,
}

impl AppContext {
    pub fn new(config: &ServerConfig) -> Result<Self, String> {
        Ok(Self {
            router: Router::new(config)?,
            max_body_size: config.global.max_body_size,
            error_pages: config.error_pages.clone(),
        })
    }
}
