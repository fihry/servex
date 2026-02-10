use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct Headers {
    headers: HashMap<String, String>,
}

impl Headers {
    pub fn new() -> Self {
        Self {
            headers: HashMap::new(),
        }
    }
}
