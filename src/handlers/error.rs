use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

pub fn load_error_page(error_pages: &HashMap<u16, PathBuf>, code: u16, reason: &str) -> String {
    if let Some(path) = error_pages.get(&code) {
        if let Ok(content) = fs::read_to_string(path) {
            return content;
        }
    }

    format!(
        "<!doctype html><html><head><title>{0} {1}</title></head><body><h1>{0} {1}</h1></body></html>",
        code, reason
    )
}
