// this module handles configuration parsing, loading, and validation for the server configurations.

pub mod models;
pub mod parser;
pub mod loader;
pub mod validator;

// Re-export commonly used types
pub use models::{
    ServerConfig,
    GlobalConfig,
    VirtualServer,
    Route,
    CgiConfig,
    Redirect,
};

pub use loader::ConfigLoader;
pub use parser::IniParser;
pub use validator::ConfigValidator;