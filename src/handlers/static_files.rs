use std::fs;
use std::path::{Path, PathBuf};

use super::directory::build_directory_listing;
use super::{HandlerError, HttpResponse};

pub fn handle_get(
    root: &Path,
    route_path: &str,
    request_path: &str,
    index: Option<&str>,
    autoindex: bool,
) -> Result<HttpResponse, HandlerError> {
    let target = resolve_target_path(root, route_path, request_path)?;
    if !target.exists() {
        return Err(HandlerError::NotFound);
    }

    if target.is_file() {
        let body = fs::read(&target).map_err(|_| HandlerError::Internal)?;
        let content_type = mime_from_path(&target);
        return Ok(HttpResponse::ok(body, content_type));
    }

    if target.is_dir() {
        if let Some(index_file) = index {
            let index_path = target.join(index_file);
            if index_path.is_file() {
                let body = fs::read(&index_path).map_err(|_| HandlerError::Internal)?;
                let content_type = mime_from_path(&index_path);
                return Ok(HttpResponse::ok(body, content_type));
            }
        }
        if autoindex {
            return Ok(HttpResponse::ok(
                build_directory_listing(&target).into_bytes(),
                "text/html; charset=utf-8",
            ));
        }
        return Err(HandlerError::Forbidden);
    }

    Err(HandlerError::NotFound)
}

pub fn resolve_target_path(root: &Path, route_path: &str, request_path: &str) -> Result<PathBuf, HandlerError> {
    let suffix = if route_path == "/" {
        request_path
    } else {
        request_path
            .strip_prefix(route_path)
            .ok_or(HandlerError::NotFound)?
    };
    let relative = suffix.trim_start_matches('/');
    Ok(root.join(relative))
}

pub fn mime_from_path(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "application/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "txt" => "text/plain; charset=utf-8",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        _ => "application/octet-stream",
    }
}
