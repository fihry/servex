mod config;
mod http;
mod routing;
mod runtime;

use config::loader::ConfigLoader;

fn main() {
    if let Err(err) = run() {
        eprintln!("server stopped: {}", err);
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let config = ConfigLoader::load("application.conf")?;
    #[cfg(debug_assertions)]
    {
        println!("the config: \n{}", config);
    }
    config::validator::ConfigValidator::validate(&config)?;
    runtime::run(config)
}
