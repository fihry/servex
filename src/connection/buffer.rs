use std::collections::VecDeque;

pub struct ConnectionBuffer {
    pub read: Vec<u8>,
    pub write: Vec<u8>,
    pub queued_writes: VecDeque<Vec<u8>>,
}

impl ConnectionBuffer {
    pub fn new() -> Self {
        Self {
            read: Vec::new(),
            write: Vec::new(),
            queued_writes: VecDeque::new(),
        }
    }

    pub fn has_pending_write(&self) -> bool {
        !self.write.is_empty() || !self.queued_writes.is_empty()
    }

    pub fn enqueue_write(&mut self, bytes: Vec<u8>) {
        if self.write.is_empty() {
            self.write = bytes;
        } else {
            self.queued_writes.push_back(bytes);
        }
    }

    pub fn promote_next_write(&mut self) -> bool {
        if self.write.is_empty() {
            if let Some(next) = self.queued_writes.pop_front() {
                self.write = next;
                return true;
            }
        }
        false
    }
}
