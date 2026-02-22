use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::http::parser::multipart::parse_multipart;

use super::static_files::resolve_target_path;
use super::{HandlerError, HttpResponse};

pub fn handle_post(
    root: &Path,
    route_path: &str,
    request_path: &str,
    upload_dir: Option<&Path>,
    content_type: Option<&str>,
    body: &[u8],
) -> Result<HttpResponse, HandlerError> {
    let dir = if let Some(upload_dir) = upload_dir {
        upload_dir.to_path_buf()
    } else {
        resolve_target_path(root, route_path, request_path)?
    };

    let (payload, extension) = extract_payload_and_extension(content_type, body);
    fs::create_dir_all(&dir).map_err(|_| HandlerError::Internal)?;
    let filename = format!("upload-{}{}", timestamp_millis(), extension.unwrap_or_default());
    let path = dir.join(&filename);
    fs::write(&path, &payload).map_err(|_| HandlerError::Internal)?;
    let payload = format!("created={}\n", filename).into_bytes();
    Ok(HttpResponse::created(payload, "text/plain; charset=utf-8"))
}

pub fn handle_delete(
    root: &Path,
    route_path: &str,
    request_path: &str,
    upload_dir: Option<&Path>,
) -> Result<HttpResponse, HandlerError> {
    let target = if let Some(upload_dir) = upload_dir {
        let suffix = if route_path == "/" {
            request_path
        } else {
            request_path
                .strip_prefix(route_path)
                .ok_or(HandlerError::NotFound)?
        };
        upload_dir.join(suffix.trim_start_matches('/'))
    } else {
        resolve_target_path(root, route_path, request_path)?
    };
    if !target.exists() {
        return Err(HandlerError::NotFound);
    }
    if target.is_dir() {
        return Err(HandlerError::Forbidden);
    }

    fs::remove_file(target).map_err(|_| HandlerError::Internal)?;
    Ok(HttpResponse::no_content())
}

fn timestamp_millis() -> u128 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_millis(),
        Err(_) => 0,
    }
}

fn extract_payload_and_extension(
    content_type: Option<&str>,
    body: &[u8],
) -> (Vec<u8>, Option<String>) {
    if let Some(content_type) = content_type {
        if let Some(boundary) = parse_boundary(content_type) {
            if let Ok(parts) = parse_multipart(body, &boundary) {
                if let Some(first) = parts.first() {
                    let ext = first
                        .headers
                        .get("content-disposition")
                        .and_then(filename_from_disposition)
                        .and_then(extension_from_filename);
                    return (first.data.clone(), ext);
                }
            }
        }

        if let Some(ext) = extension_from_content_type(content_type) {
            return (body.to_vec(), Some(ext.to_string()));
        }
    }
    (body.to_vec(), None)
}

fn parse_boundary(content_type: &str) -> Option<String> {
    content_type
        .split(';')
        .map(|part| part.trim())
        .find_map(|part| part.strip_prefix("boundary=").map(|value| value.trim_matches('"').to_string()))
}

fn filename_from_disposition(disposition: &str) -> Option<&str> {
    disposition
        .split(';')
        .map(|part| part.trim())
        .find_map(|part| part.strip_prefix("filename=").map(|value| value.trim_matches('"')))
}

fn extension_from_filename(filename: &str) -> Option<String> {
    let (_, ext) = filename.rsplit_once('.')?;
    if ext.is_empty() {
        None
    } else {
        Some(format!(".{}", ext))
    }
}

fn extension_from_content_type(content_type: &str) -> Option<&'static str> {
    let normalized = content_type.trim().to_ascii_lowercase();
    let mime = normalized.split(';').next().unwrap_or("");
    match mime {
        "image/jpeg" => Some(".jpg"),
        "image/png" => Some(".png"),
        "image/webp" => Some(".webp"),
        "image/gif" => Some(".gif"),
        "text/plain" => Some(".txt"),
        "application/json" => Some(".json"),
        "text/html" => Some(".html"),
        "application/pdf" => Some(".pdf"),
        _ => None,
    }
}
