use mio::Token;

pub const LISTENER_TOKEN_BASE: usize = 0;
pub const CONNECTION_TOKEN_BASE: usize = 1024;

pub struct TokenFactory {
    next: usize,
}

impl TokenFactory {
    pub fn new() -> Self {
        Self {
            next: CONNECTION_TOKEN_BASE,
        }
    }

    pub fn next(&mut self) -> Token {
        let token = Token(self.next);
        self.next += 1;
        token
    }
}
