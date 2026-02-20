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

    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        self.headers.insert(normalize_key(&key), value.into());
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.headers.get(&normalize_key(key)).map(|value| value.as_str())
    }

    pub fn contains(&self, key: &str) -> bool {
        self.headers.contains_key(&normalize_key(key))
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &String)> {
        self.headers.iter()
    }
}

fn normalize_key(key: &str) -> String {
    key.trim().to_ascii_lowercase()
}
