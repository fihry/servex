use mio::net::TcpListener;
use mio::Token;
use std::net::SocketAddr;

pub struct Listener {
    pub token: Token,
    pub listener: TcpListener,
    pub addr: SocketAddr,
}

impl Listener {
    pub fn new(token: Token, addr: SocketAddr, listener: TcpListener) -> Self {
        Self {
            token,
            listener,
            addr,
        }
    }
}
