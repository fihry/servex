use mio::net::TcpStream;
use mio::{Interest, Registry, Token};
use std::io::{self, Read, Write};
use std::time::Instant;

use super::buffer::ConnectionBuffer;
use super::handler::try_build_response;
use super::state::ConnectionState;

const READ_CHUNK_SIZE: usize = 8 * 1024;

pub struct Connection {
    pub token: Token,
    pub stream: TcpStream,
    pub state: ConnectionState,
    pub buffer: ConnectionBuffer,
    pub last_active: Instant,
}

impl Connection {
    pub fn new(token: Token, stream: TcpStream) -> Self {
        Self {
            token,
            stream,
            state: ConnectionState::Reading,
            buffer: ConnectionBuffer::new(),
            last_active: Instant::now(),
        }
    }

    pub fn register(&mut self, registry: &Registry) -> io::Result<()> {
        registry.register(&mut self.stream, self.token, Interest::READABLE)
    }

    pub fn reregister(&mut self, registry: &Registry, interest: Interest) -> io::Result<()> {
        registry.reregister(&mut self.stream, self.token, interest)
    }

    pub fn readable(&mut self) -> io::Result<Option<ConnectionState>> {
        let mut buf = [0u8; READ_CHUNK_SIZE];
        loop {
            match self.stream.read(&mut buf) {
                Ok(0) => {
                    self.state = ConnectionState::Closed;
                    return Ok(Some(self.state));
                }
                Ok(n) => {
                    self.last_active = Instant::now();
                    self.buffer.read.extend_from_slice(&buf[..n]);
                    if let Some(response) = try_build_response(&mut self.buffer.read) {
                        self.buffer.write = response;
                        self.state = ConnectionState::Writing;
                        return Ok(Some(self.state));
                    }
                }
                Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => {
                    return Ok(None);
                }
                Err(err) => {
                    self.state = ConnectionState::Closed;
                    return Err(err);
                }
            }
        }
    }

    pub fn writable(&mut self) -> io::Result<Option<ConnectionState>> {
        while !self.buffer.write.is_empty() {
            match self.stream.write(&self.buffer.write) {
                Ok(0) => {
                    self.state = ConnectionState::Closed;
                    return Ok(Some(self.state));
                }
                Ok(n) => {
                    self.last_active = Instant::now();
                    self.buffer.write.drain(..n);
                }
                Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => {
                    return Ok(None);
                }
                Err(err) => {
                    self.state = ConnectionState::Closed;
                    return Err(err);
                }
            }
        }

        self.state = ConnectionState::Closed;
        Ok(Some(self.state))
    }
}
