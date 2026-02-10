mod config;
mod connection;
mod core;
mod http;

use config::loader::ConfigLoader;
use connection::stream::HandleConnection;
use mio::{ Events, Interest, Poll, Token };
use mio::net::{ TcpListener };
use std::io;
use std::net::{ AddrParseError, SocketAddr };
use std::thread;

fn main() -> Result<(), String> {
    // Load configuration
    macro_rules! hey {
    ($address:expr) => {
        println!("address: {}", $address);
        println!("===================================");
    };
}

    let config = ConfigLoader::load("application.conf")?;
    config::validator::ConfigValidator::validate(&config)?;
    for server in config.servers {
        for port in server.ports {
            let address: SocketAddr = format!("{}:{}", server.host, port)
                .parse()
                .map_err(|e: AddrParseError| e.to_string())?;

            hey!(address);

            thread::spawn(move || {
                let listener = TcpListener::bind(address).unwrap();
                println!("Listening on {}", address);

                loop {
                    match listener.accept() {
                        Ok((stream, addr)) => {
                            println!("Connection from {}", addr);
                            let _ = HandleConnection::handle_connection(stream);
                        }
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                            // Nothing to accept right now
                            std::thread::sleep(std::time::Duration::from_millis(50));
                        }
                        Err(e) => {
                            eprintln!("accept error: {}", e);
                            break;
                        }
                    }
                }

            });
        }
    }

    // prevent main from exiting
    loop {
        thread::park();
    }
}
