mod config;
mod http;
mod routing;

use config::loader::ConfigLoader;

fn main() -> Result<(), String> {
    let config = ConfigLoader::load("application.conf")?;
    config::validator::ConfigValidator::validate(&config)?;
    #[cfg(debug_assertions)]
    {
        println!("the config: \n{}", config);
    }
    Ok(())
}
