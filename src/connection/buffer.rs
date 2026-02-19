pub struct ConnectionBuffer {
    pub read: Vec<u8>,
    pub write: Vec<u8>,
}

impl ConnectionBuffer {
    pub fn new() -> Self {
        Self {
            read: Vec::new(),
            write: Vec::new(),
        }
    }

    pub fn has_pending_write(&self) -> bool {
        !self.write.is_empty()
    }
}
