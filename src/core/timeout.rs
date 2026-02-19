use std::time::{Duration, Instant};

pub struct Timeout {
    timeout: Duration,
}

impl Timeout {
    pub fn new(seconds: u64) -> Self {
        Self {
            timeout: Duration::from_secs(seconds),
        }
    }

    pub fn expired(&self, last_active: Instant) -> bool {
        last_active.elapsed() > self.timeout
    }
}
