mod config;

use config::loader::ConfigLoader;

fn main() -> Result<(), String> {
    // Load configuration
    let config = ConfigLoader::load("application.conf")?;
    println!("Loaded {} servers", config.servers.len());
    for server in &config.servers {
        println!(" Server: {} on {:?}", server.host, server.ports);
        println!("Routes: {}", server.routes.len());
    }
    Ok(())
}
