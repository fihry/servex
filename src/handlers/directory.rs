use std::fs;
use std::path::Path;

pub fn build_directory_listing(path: &Path) -> String {
    let mut entries = match fs::read_dir(path) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().into_string().ok())
            .collect::<Vec<String>>(),
        Err(_) => Vec::new(),
    };
    entries.sort();

    let mut html =
        String::from("<!doctype html><html><head><title>Index</title></head><body><h1>Directory Listing</h1><ul>");
    for entry in entries {
        html.push_str("<li>");
        html.push_str(&entry);
        html.push_str("</li>");
    }
    html.push_str("</ul></body></html>");
    html
}
