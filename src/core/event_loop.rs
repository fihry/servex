use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use mio::event::Event;
use mio::{Events, Interest, Poll, Token};
use mio::net::TcpListener;

use crate::config::models::ServerConfig;
use crate::connection::Connection;

use super::listener::Listener;
use super::registry::{TokenFactory, LISTENER_TOKEN_BASE};
use super::timeout::Timeout;

pub struct EventLoop {
    poll: Poll,
    listeners: Vec<Listener>,
    connections: HashMap<Token, Connection>,
    token_factory: TokenFactory,
    timeout: Timeout,
}

impl EventLoop {
    pub fn new(config: &ServerConfig) -> io::Result<Self> {
        let poll = Poll::new()?;
        let listeners = Self::bind_listeners(&poll, config)?;

        Ok(Self {
            poll,
            listeners,
            connections: HashMap::new(),
            token_factory: TokenFactory::new(),
            timeout: Timeout::new(config.global.timeout),
        })
    }

    pub fn run(&mut self) -> io::Result<()> {
        let mut events = Events::with_capacity(1024);

        loop {
            self.poll.poll(&mut events, Some(Duration::from_millis(100)))?;
            for event in events.iter() {
                if self.is_listener_event(event) {
                    self.accept_connections(event)?;
                } else {
                    self.handle_connection_event(event)?;
                }
            }
            self.close_expired();
        }
    }

    fn bind_listeners(poll: &Poll, config: &ServerConfig) -> io::Result<Vec<Listener>> {
        let mut listeners = Vec::new();
        let mut token = LISTENER_TOKEN_BASE;

        for server in &config.servers {
            for port in &server.ports {
                let addr: SocketAddr = format!("{}:{}", server.host, port).parse().map_err(|err| {
                    io::Error::new(io::ErrorKind::InvalidInput, err)
                })?;
                let mut listener = TcpListener::bind(addr)?;
                poll.registry()
                    .register(&mut listener, Token(token), Interest::READABLE)?;
                println!("Listening on {}", addr);
                listeners.push(Listener::new(Token(token), addr, listener));
                token += 1;
            }
        }

        Ok(listeners)
    }

    fn is_listener_event(&self, event: &Event) -> bool {
        self.listeners.iter().any(|listener| listener.token == event.token())
    }

    fn accept_connections(&mut self, event: &Event) -> io::Result<()> {
        let listener = match self.listeners.iter_mut().find(|l| l.token == event.token()) {
            Some(listener) => listener,
            None => return Ok(()),
        };

        loop {
            match listener.listener.accept() {
                Ok((stream, _addr)) => {
                    let token = self.token_factory.next();
                    let mut connection = Connection::new(token, stream);
                    connection.register(self.poll.registry())?;
                    self.connections.insert(token, connection);
                }
                Err(ref err) if err.kind() == io::ErrorKind::WouldBlock => {
                    break;
                }
                Err(err) => return Err(err),
            }
        }

        Ok(())
    }

    fn handle_connection_event(&mut self, event: &Event) -> io::Result<()> {
        let token = event.token();
        let mut remove = false;

        if let Some(connection) = self.connections.get_mut(&token) {
            if event.is_readable() {
                // kljdas
                if let Some(state) = connection.readable()? {
                    if state == crate::connection::state::ConnectionState::Writing {
                        connection.reregister(self.poll.registry(), Interest::WRITABLE)?;
                    }
                    if state == crate::connection::state::ConnectionState::Closed {
                        remove = true;
                    }
                }
            }

            if event.is_writable() {
                if let Some(state) = connection.writable()? {
                    if state == crate::connection::state::ConnectionState::Closed {
                        remove = true;
                    }
                }
            }
        }

        if remove {
            if let Some(mut connection) = self.connections.remove(&token) {
                let _ = self.poll.registry().deregister(&mut connection.stream);
            }
        }

        Ok(())
    }

    fn close_expired(&mut self) {
        let tokens_to_close: Vec<Token> = self.connections
            .iter()
            .filter(|(_, conn)| self.timeout.expired(conn.last_active))
            .map(|(token, _)| *token)
            .collect();

        for token in tokens_to_close {
            if let Some(mut connection) = self.connections.remove(&token) {
                let _ = self.poll.registry().deregister(&mut connection.stream);
            }
        }
    }
}
