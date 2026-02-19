mod config;
mod connection;
mod core;
mod http;

use config::loader::ConfigLoader;
use core::event_loop::EventLoop;

fn main() -> Result<(), String> {
    let config = ConfigLoader::load("application.conf")?;
    config::validator::ConfigValidator::validate(&config)?;

    let mut event_loop = EventLoop::new(&config).map_err(|err| err.to_string())?;
    event_loop.run().map_err(|err| err.to_string())?;
    Ok(())
}
