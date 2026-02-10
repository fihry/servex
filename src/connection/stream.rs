use std::io::{BufRead, BufReader};
use mio::net::TcpStream;

pub struct HandleConnection;

impl HandleConnection {
    pub fn handle_connection(stream: TcpStream) -> Result<(), String> {
        let buf_reader = BufReader::new(&stream);
        let http_request: Vec<_> = buf_reader.lines()
            .map(|result| result.unwrap_or_default())
            .take_while(|line| !line.is_empty())
            .collect();

        println!("Received request:\n{}", http_request.join("\n"));
        Ok(())
    }
}