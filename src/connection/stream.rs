use mio::net::TcpStream;
use mio::{Interest, Registry, Token};
use std::io::{self, Read, Write};
use std::sync::Arc;
use std::time::Instant;

use super::buffer::ConnectionBuffer;
use super::handler::RequestHandler;
use super::state::ConnectionState;

const READ_CHUNK_SIZE: usize = 8 * 1024;

pub struct Connection {
    pub token: Token,
    pub stream: TcpStream,
    pub state: ConnectionState,
    pub buffer: ConnectionBuffer,
    pub last_active: Instant,
    pub close_after_write: bool,
    pub request_handler: Arc<RequestHandler>,
}

impl Connection {
    pub fn new(token: Token, stream: TcpStream, request_handler: Arc<RequestHandler>) -> Self {
        Self {
            token,
            stream,
            state: ConnectionState::Reading,
            buffer: ConnectionBuffer::new(),
            last_active: Instant::now(),
            close_after_write: false,
            request_handler,
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
                    let mut queued_any = false;
                    loop {
                        match self.request_handler.try_build_response(&mut self.buffer.read) {
                            Some(response) => {
                                queued_any = true;
                                if response.close_after_write {
                                    self.close_after_write = true;
                                    self.buffer.read.clear();
                                }
                                self.buffer.enqueue_write(response.bytes);
                                if self.close_after_write {
                                    break;
                                }
                            }
                            None => break,
                        }
                    }

                    if queued_any {
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
        loop {
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

            if !self.buffer.promote_next_write() {
                break;
            }
        }

        self.state = if self.close_after_write { ConnectionState::Closed } else { ConnectionState::Reading };
        Ok(Some(self.state))
    }
}
