mod core;

#[cfg(test)]
mod handlers {
    pub(super) use super::core::{
        Target, build_response, handle_matched, resolve_delete_target, resolve_relative_path,
        resolve_target_path, should_keep_alive, store_upload,
    };
}

#[cfg(test)]
mod response {
    pub(super) use super::core::{build_error_response, detect_content_type, status_from_code};
}

#[cfg(test)]
mod session {
    pub(super) use super::core::attach_session_cookie;
}

#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};
use std::io::{ErrorKind, Read, Write};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use mio::net::{TcpListener, TcpStream};
use mio::{Events, Interest, Poll, Token};

use crate::config::models::ServerConfig;
use crate::http::models::status::Status;
use crate::http::parser::chunked::ChunkedError;
use crate::http::parser::request::{RequestParseError, parse_request};
use crate::routing::Router;

use core::{build_error_response, build_response, should_keep_alive};

const READ_BUFFER_SIZE: usize = 8 * 1024;

struct Connection {
    socket: TcpStream,
    read_buf: Vec<u8>,
    write_buf: Vec<u8>,
    close_after_write: bool,
    peer_closed: bool,
    last_active: Instant,
}

struct ListenerEntry {
    listener: TcpListener,
}

impl Connection {
    fn new(socket: TcpStream) -> Self {
        Self {
            socket,
            read_buf: Vec::new(),
            write_buf: Vec::new(),
            close_after_write: false,
            peer_closed: false,
            last_active: Instant::now(),
        }
    }
}

pub fn run(config: ServerConfig) -> Result<(), String> {
    let router = Router::new(&config)?;
    let mut poll = Poll::new().map_err(|e| format!("failed to create poll: {}", e))?;
    let mut events = Events::with_capacity(1024);

    let mut listeners = HashMap::<Token, ListenerEntry>::new();
    let mut connections = HashMap::<Token, Connection>::new();
    let mut next_token = 0usize;
    let mut sessions = HashMap::<String, Instant>::new();

    let mut listeners_by_addr: HashSet<SocketAddr> = HashSet::new();
    for port in &config.server.ports {
        let addr: SocketAddr = format!("{}:{}", config.server.host, port)
            .parse()
            .map_err(|e| format!("invalid listener addr for {}:{}: {}", config.server.host, port, e))?;
        listeners_by_addr.insert(addr);
    }

    for addr in listeners_by_addr {
        let mut listener =
            TcpListener::bind(addr).map_err(|e| format!("failed to bind {}: {}", addr, e))?;
        let token = Token(next_token);
        next_token += 1;
        poll.registry()
            .register(&mut listener, token, Interest::READABLE)
            .map_err(|e: std::io::Error| format!("failed to register listener {}: {}", addr, e))?;
        listeners.insert(
            token,
            ListenerEntry {
                listener,
            },
        );
    }

    loop {
        poll.poll(&mut events, Some(Duration::from_millis(200)))
            .map_err(|e| format!("poll failed: {}", e))?;

        for event in &events {
            let token = event.token();
            if listeners.contains_key(&token) {
                match accept_clients(
                    &mut listeners,
                    &mut connections,
                    poll.registry(),
                    token,
                    &mut next_token,
                ){
                    Ok(_) => {},
                    Err(err) => println!("error accepting the clients: {:?}\nerr: {:?}", token, err),
                };
                continue;
            }

            if let Some(conn) = connections.get_mut(&token) {
                if event.is_readable() {
                    read_from_client(conn, &config, &router, &mut sessions);
                }
                if event.is_writable() {
                    write_to_client(conn);
                }
            }
        }

        let mut to_remove = Vec::new();
        let now = Instant::now();
        for (token, conn) in &mut connections {
            if now.duration_since(conn.last_active).as_secs() > config.global.timeout {
                if conn.write_buf.is_empty() {
                    conn.write_buf = build_error_response(&config, Status::REQUEST_TIMEOUT, false);
                }
                conn.close_after_write = true;
            }

            if conn.peer_closed && conn.write_buf.is_empty() {
                to_remove.push(*token);
                continue;
            }

            if conn.close_after_write && conn.write_buf.is_empty() {
                to_remove.push(*token);
                continue;
            }

            let interest = if conn.write_buf.is_empty() {
                Interest::READABLE
            } else {
                Interest::READABLE.add(Interest::WRITABLE)
            };
            if let Err(e) = poll.registry().reregister(&mut conn.socket, *token, interest) {
                eprintln!("reregister failed for {:?}: {}", token, e);
                to_remove.push(*token);
            }
        }

        for token in to_remove {
            if let Some(mut conn) = connections.remove(&token) {
                let _ = poll.registry().deregister(&mut conn.socket);
            }
        }
    }
}

fn accept_clients(
    listeners: &mut HashMap<Token, ListenerEntry>,
    connections: &mut HashMap<Token, Connection>,
    registry: &mio::Registry,
    listener_token: Token,
    next_token: &mut usize,
) -> Result<(), String> {
    let entry = listeners
        .get_mut(&listener_token)
        .ok_or_else(|| "missing listener for token".to_string())?;

    loop {
        match entry.listener.accept() {
            Ok((mut stream, _)) => {
                let token = Token(*next_token);
                *next_token += 1;
                registry
                    .register(&mut stream, token, Interest::READABLE)
                    .map_err(|e| format!("failed to register connection: {}", e))?;
                connections.insert(token, Connection::new(stream));
            }
            Err(err) if err.kind() == ErrorKind::WouldBlock => break,
            Err(err) => {
                eprintln!("accept failed: {}", err);
                break;
            }
        }
    }
    Ok(())
}

fn read_from_client(
    conn: &mut Connection,
    config: &ServerConfig,
    router: &Router,
    sessions: &mut HashMap<String, Instant>,
) {
    let mut buffer = [0u8; READ_BUFFER_SIZE];
    match conn.socket.read(&mut buffer) {
        Ok(0) => {
            conn.peer_closed = true;
        }
        Ok(n) => {
            conn.last_active = Instant::now();
            conn.read_buf.extend_from_slice(&buffer[..n]);
        }
        Err(err) if err.kind() == ErrorKind::WouldBlock => {}
        Err(_) => {
            conn.close_after_write = true;
        }
    }

    loop {
        match parse_request(&conn.read_buf) {
            Ok((request, consumed)) => {
                conn.read_buf.drain(..consumed);
                let keep_alive = should_keep_alive(config, request.version.as_str(), &request);
                let response = build_response(
                    config,
                    router,
                    sessions,
                    &request,
                    keep_alive,
                );
                conn.write_buf.extend_from_slice(&response);
                if !keep_alive {
                    conn.close_after_write = true;
                }
            }
            Err(RequestParseError::Incomplete) => break,
            Err(RequestParseError::Chunked(ChunkedError::Incomplete)) => break,
            Err(_) => {
                let response = build_error_response(config, Status::BAD_REQUEST, false);
                conn.write_buf.extend_from_slice(&response);
                conn.close_after_write = true;
                conn.read_buf.clear();
                break;
            }
        }
    }
}

fn write_to_client(conn: &mut Connection) {
    if conn.write_buf.is_empty() {
        return;
    }
    match conn.socket.write(&conn.write_buf) {
        Ok(0) => {
            conn.close_after_write = true;
        }
        Ok(n) => {
            conn.last_active = Instant::now();
            conn.write_buf.drain(..n);
        }
        Err(err) if err.kind() == ErrorKind::WouldBlock => {}
        Err(_) => {
            conn.close_after_write = true;
        }
    }
}
