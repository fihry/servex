mod config;
mod http;
mod routing;
mod runtime;

use config::loader::ConfigLoader;

fn main() -> Result<(), String> {
    let config = ConfigLoader::load("application.conf")?;
    #[cfg(debug_assertions)]
    {
        println!("the config: \n{}", config);
    }
    config::validator::ConfigValidator::validate(&config)?;
    runtime::run(config)
}
