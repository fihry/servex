use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::config::models::{Redirect, Route, ServerConfig};
use crate::config::server::Server;
use crate::http::models::headers::Headers;
use crate::http::models::method::Method;
use crate::http::models::request::Request;
use crate::http::models::status::Status;
use crate::routing::Router;

use super::handlers::{
    build_response, handle_matched, resolve_delete_target, resolve_relative_path,
    resolve_target_path, should_keep_alive, store_upload, Target,
};
use super::response::{build_error_response, detect_content_type, status_from_code};
use super::session::attach_session_cookie;

fn make_config(session_data: &[(&str, &str)]) -> ServerConfig {
    let mut config = ServerConfig::default();
    let mut data = HashMap::new();
    for (k, v) in session_data {
        data.insert((*k).to_string(), (*v).to_string());
    }
    if !data.is_empty() {
        config
            .sessions
            .inject(&data)
            .expect("session config should be valid");
    }
    config
}

fn make_request(cookie: Option<&str>) -> Request {
    let mut headers = Headers::new();
    headers.insert("host", "localhost");
    if let Some(value) = cookie {
        headers.insert("cookie", value);
    }
    Request::new(
        Method::Get,
        "/ok".to_string(),
        "HTTP/1.1".to_string(),
        headers,
        Vec::new(),
    )
}

fn make_request_with(
    method: Method,
    path: &str,
    version: &str,
    body: Vec<u8>,
    extra_headers: &[(&str, &str)],
) -> Request {
    let mut headers = Headers::new();
    for (k, v) in extra_headers {
        headers.insert(*k, *v);
    }
    Request::new(method, path.to_string(), version.to_string(), headers, body)
}

fn unique_path(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("servex_runtime_{prefix}_{nanos}"))
}

fn make_route(path: &str, methods: &[&str]) -> Route {
    Route {
        path: path.to_string(),
        methods: methods.iter().map(|m| (*m).to_string()).collect(),
        root: None,
        index: None,
        redirect: None,
        cgi: None,
        upload_dir: None,
        autoindex: false,
        max_file_size: None,
    }
}

fn make_config_with_route(root: &Path, route: Route) -> ServerConfig {
    let mut config = ServerConfig::default();
    config
        .sessions
        .inject(&HashMap::from([
            ("enabled".to_string(), "false".to_string()),
            ("timeout".to_string(), "60".to_string()),
            ("cookie_name".to_string(), "S".to_string()),
            ("secure".to_string(), "false".to_string()),
            ("http_only".to_string(), "true".to_string()),
        ]))
        .expect("session config");
    config.server = Server {
        name: "main".to_string(),
        server_names: vec!["localhost".to_string()],
        host: "127.0.0.1".to_string(),
        ports: vec![8080],
        root: root.to_path_buf(),
        routes: vec![route],
    };
    config
}

fn status_line(response: &[u8]) -> String {
    String::from_utf8_lossy(response)
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_string()
}

fn response_has_header(response: &[u8], key: &str) -> bool {
    let needle = format!("{key}:");
    String::from_utf8_lossy(response)
        .lines()
        .any(|line| line.starts_with(&needle))
}

#[test]
fn session_cookie_not_added_when_sessions_disabled() {
    let config = make_config(&[
        ("enabled", "false"),
        ("timeout", "60"),
        ("cookie_name", "LOCALSERVER_SESSION"),
        ("secure", "false"),
        ("http_only", "true"),
    ]);
    let request = make_request(None);
    let mut sessions = HashMap::new();
    let mut response_headers = Vec::new();

    attach_session_cookie(&config, &mut sessions, &request, &mut response_headers);

    assert!(response_headers.is_empty());
    assert!(sessions.is_empty());
}

#[test]
fn session_cookie_uses_configured_name_and_flags() {
    let config = make_config(&[
        ("enabled", "true"),
        ("timeout", "60"),
        ("cookie_name", "MY_SESSION"),
        ("secure", "true"),
        ("http_only", "true"),
    ]);
    let request = make_request(None);
    let mut sessions = HashMap::new();
    let mut response_headers = Vec::new();

    attach_session_cookie(&config, &mut sessions, &request, &mut response_headers);

    assert_eq!(response_headers.len(), 1);
    let (name, value) = &response_headers[0];
    assert_eq!(name, "Set-Cookie");
    assert!(value.starts_with("MY_SESSION="));
    assert!(value.contains("; HttpOnly"));
    assert!(value.contains("; Secure"));
}

#[test]
fn existing_cookie_keeps_session_without_new_set_cookie() {
    let config = make_config(&[
        ("enabled", "true"),
        ("timeout", "60"),
        ("cookie_name", "LOCALSERVER_SESSION"),
        ("secure", "false"),
        ("http_only", "true"),
    ]);
    let request = make_request(Some("LOCALSERVER_SESSION=abc123"));
    let mut sessions = HashMap::new();
    sessions.insert("abc123".to_string(), Instant::now());
    let mut response_headers = Vec::new();

    attach_session_cookie(&config, &mut sessions, &request, &mut response_headers);

    assert!(response_headers.is_empty());
    assert!(sessions.contains_key("abc123"));
}

#[test]
fn cookie_header_without_target_cookie_emits_set_cookie() {
    let config = make_config(&[
        ("enabled", "true"),
        ("timeout", "60"),
        ("cookie_name", "LOCALSERVER_SESSION"),
        ("secure", "false"),
        ("http_only", "true"),
    ]);
    let request = make_request(Some("OTHER=value"));
    let mut sessions = HashMap::new();
    let mut response_headers = Vec::new();

    attach_session_cookie(&config, &mut sessions, &request, &mut response_headers);

    assert_eq!(response_headers.len(), 1);
    assert_eq!(response_headers[0].0, "Set-Cookie");
    assert_eq!(sessions.len(), 1);
}

#[test]
fn should_keep_alive_respects_http10_and_connection_header() {
    let mut config = ServerConfig::default();
    config.global.keep_alive = true;

    let req_close = make_request_with(
        Method::Get,
        "/",
        "HTTP/1.0",
        Vec::new(),
        &[("host", "localhost")],
    );
    assert!(!should_keep_alive(&config, "HTTP/1.0", &req_close));

    let req_keep = make_request_with(
        Method::Get,
        "/",
        "HTTP/1.0",
        Vec::new(),
        &[("host", "localhost"), ("connection", "keep-alive")],
    );
    assert!(should_keep_alive(&config, "HTTP/1.0", &req_keep));
}

#[test]
fn should_keep_alive_honors_global_flag_and_http11_close() {
    let mut config = ServerConfig::default();
    config.global.keep_alive = false;
    let req = make_request_with(
        Method::Get,
        "/",
        "HTTP/1.1",
        Vec::new(),
        &[("host", "localhost")],
    );
    assert!(!should_keep_alive(&config, "HTTP/1.1", &req));

    config.global.keep_alive = true;
    let req_close = make_request_with(
        Method::Get,
        "/",
        "HTTP/1.1",
        Vec::new(),
        &[("host", "localhost"), ("connection", "close")],
    );
    assert!(!should_keep_alive(&config, "HTTP/1.1", &req_close));
}

#[test]
fn resolve_relative_path_blocks_parent_directory_traversal() {
    let root = PathBuf::from("/tmp");
    let err = resolve_relative_path(&root, "/static", "/static/../secret.txt")
        .expect_err("path traversal must be blocked");
    assert_eq!(err, Status::FORBIDDEN);
}

#[test]
fn resolve_target_path_returns_index_file_for_directory() {
    let root = unique_path("index_root");
    let blog_root = root.join("blog");
    fs::create_dir_all(&blog_root).expect("create dir");
    fs::write(blog_root.join("index.html"), "hello").expect("write file");

    let target = resolve_target_path(&blog_root, "/blog", "/blog", Some("index.html"), false)
        .expect("index should resolve");
    match target {
        Target::File(path) => assert!(path.ends_with("blog/index.html")),
        Target::DirectoryListing(_) => panic!("expected file target"),
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn resolve_target_path_returns_forbidden_when_autoindex_disabled() {
    let root = unique_path("autoindex_off");
    fs::create_dir_all(root.join("assets")).expect("create dir");

    let err = match resolve_target_path(&root, "/assets", "/assets", None, false) {
        Ok(_) => panic!("directory should be forbidden without index"),
        Err(err) => err,
    };
    assert_eq!(err, Status::FORBIDDEN);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn resolve_target_path_returns_directory_listing_when_enabled() {
    let root = unique_path("autoindex_on");
    fs::create_dir_all(root.join("assets")).expect("create dir");

    let target = resolve_target_path(&root, "/assets", "/assets", None, true)
        .expect("directory listing should be returned");
    assert!(matches!(target, Target::DirectoryListing(_)));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn store_upload_rejects_missing_directory() {
    let request = make_request_with(
        Method::Post,
        "/upload",
        "HTTP/1.1",
        b"abc".to_vec(),
        &[("host", "localhost")],
    );
    let err = store_upload(&unique_path("missing_upload_dir"), &request)
        .expect_err("missing dir should fail");
    assert_eq!(err, Status::NOT_FOUND);
}

#[test]
fn store_upload_rejects_when_upload_path_is_file() {
    let path = unique_path("upload_file");
    fs::write(&path, "not a dir").expect("create file");
    let request = make_request_with(
        Method::Post,
        "/upload",
        "HTTP/1.1",
        b"abc".to_vec(),
        &[("host", "localhost")],
    );
    let err = store_upload(&path, &request).expect_err("file upload path should fail");
    assert_eq!(err, Status::FORBIDDEN);
    let _ = fs::remove_file(path);
}

#[test]
fn store_upload_sanitizes_multipart_filename() {
    let dir = unique_path("upload_dir");
    fs::create_dir_all(&dir).expect("create upload dir");
    let body = b"--abc\r\nContent-Disposition: form-data; name=\"file\"; filename=\"../evil.txt\"\r\n\r\npwn\r\n--abc--\r\n".to_vec();
    let request = make_request_with(
        Method::Post,
        "/upload",
        "HTTP/1.1",
        body,
        &[
            ("host", "localhost"),
            ("content-type", "multipart/form-data; boundary=abc"),
        ],
    );

    let saved = store_upload(&dir, &request).expect("upload should succeed");
    assert_eq!(saved.file_name().and_then(|v| v.to_str()), Some("evil.txt"));
    assert_eq!(fs::read(saved).expect("read saved file"), b"pwn");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn build_response_rejects_http11_request_without_host() {
    let root = unique_path("missing_host_root");
    fs::create_dir_all(&root).expect("create root");
    let config = make_config_with_route(&root, make_route("/x", &["GET"]));
    let router = Router::new(&config).expect("router");
    let request = make_request_with(Method::Get, "/x", "HTTP/1.1", Vec::new(), &[]);
    let mut sessions = HashMap::new();

    let response = build_response(&config, &router, &mut sessions, &request, false);
    assert!(status_line(&response).contains("400 Bad Request"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn build_response_applies_global_max_body_size() {
    let root = unique_path("max_body_root");
    fs::create_dir_all(&root).expect("create root");
    let mut config = make_config_with_route(&root, make_route("/x", &["POST"]));
    config.global.max_body_size = 1;
    let router = Router::new(&config).expect("router");
    let request = make_request_with(
        Method::Post,
        "/x",
        "HTTP/1.1",
        b"ab".to_vec(),
        &[("host", "localhost")],
    );
    let mut sessions = HashMap::new();

    let response = build_response(&config, &router, &mut sessions, &request, false);
    assert!(status_line(&response).contains("413 Payload Too Large"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn handle_matched_applies_route_max_file_size() {
    let config = ServerConfig::default();
    let root = unique_path("max_file_root");
    fs::create_dir_all(&root).expect("create root");
    let request = make_request_with(
        Method::Post,
        "/upload",
        "HTTP/1.1",
        b"ab".to_vec(),
        &[("host", "localhost")],
    );
    let mut sessions = HashMap::new();

    let response = handle_matched(
        &config,
        &mut sessions,
        &request,
        false,
        "/upload",
        "/upload",
        &root,
        None,
        false,
        Some(&root),
        None,
        Some(1),
    );
    assert!(status_line(&response).contains("413 Payload Too Large"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn build_response_redirect_contains_location_header() {
    let root = unique_path("redirect_root");
    fs::create_dir_all(&root).expect("create root");
    let mut route = make_route("/old", &["GET"]);
    route.redirect = Some(Redirect {
        status: 301,
        target: "/new".to_string(),
    });
    let config = make_config_with_route(&root, route);
    let router = Router::new(&config).expect("router");
    let request = make_request_with(
        Method::Get,
        "/old",
        "HTTP/1.1",
        Vec::new(),
        &[("host", "localhost")],
    );
    let mut sessions = HashMap::new();

    let response = build_response(&config, &router, &mut sessions, &request, true);
    assert!(status_line(&response).contains("301 Moved Permanently"));
    assert!(response_has_header(&response, "Location"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn build_response_head_method_allowed_by_route_still_returns_405() {
    let root = unique_path("head_root");
    fs::create_dir_all(&root).expect("create root");
    fs::write(root.join("ok"), "ok").expect("write file");
    let mut route = make_route("/ok", &["HEAD"]);
    route.index = Some("ok".to_string());
    let config = make_config_with_route(&root, route);
    let router = Router::new(&config).expect("router");
    let request = make_request_with(
        Method::Head,
        "/ok",
        "HTTP/1.1",
        Vec::new(),
        &[("host", "localhost")],
    );
    let mut sessions = HashMap::new();

    let response = build_response(&config, &router, &mut sessions, &request, false);
    assert!(status_line(&response).contains("405 Method Not Allowed"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn resolve_delete_target_blocks_parent_directory_traversal() {
    let root = PathBuf::from("/tmp");
    let err = resolve_delete_target(&root, "/uploads", "/uploads/../a.txt")
        .expect_err("traversal must be blocked");
    assert_eq!(err, Status::FORBIDDEN);
}

#[test]
fn build_response_delete_existing_file_returns_204() {
    let root = unique_path("delete_root");
    fs::create_dir_all(&root).expect("create root");
    fs::write(root.join("a.txt"), "x").expect("write file");

    let route = make_route("/uploads", &["DELETE"]);
    let config = make_config_with_route(&root, route);
    let router = Router::new(&config).expect("router");
    let request = make_request_with(
        Method::Delete,
        "/uploads/a.txt",
        "HTTP/1.1",
        Vec::new(),
        &[("host", "localhost")],
    );
    let mut sessions = HashMap::new();
    let response = build_response(&config, &router, &mut sessions, &request, false);

    assert!(status_line(&response).contains("204 No Content"));
    assert!(!root.join("a.txt").exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn build_response_redirect_with_unknown_code_falls_back_to_302() {
    let root = unique_path("redirect_fallback_root");
    fs::create_dir_all(&root).expect("create root");
    let mut route = make_route("/old", &["GET"]);
    route.redirect = Some(Redirect {
        status: 399,
        target: "/new".to_string(),
    });
    let config = make_config_with_route(&root, route);
    let router = Router::new(&config).expect("router");
    let request = make_request_with(
        Method::Get,
        "/old",
        "HTTP/1.1",
        Vec::new(),
        &[("host", "localhost")],
    );
    let mut sessions = HashMap::new();

    let response = build_response(&config, &router, &mut sessions, &request, true);
    assert!(status_line(&response).contains("302 Found"));
    assert!(response_has_header(&response, "Location"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn build_error_response_uses_custom_error_page_when_present() {
    let root = unique_path("custom_error_root");
    fs::create_dir_all(&root).expect("create root");
    let error_file = root.join("404.html");
    fs::write(&error_file, "<h1>custom not found</h1>").expect("write error file");

    let mut config = ServerConfig::default();
    config.error_pages.insert(404, error_file);
    let response = build_error_response(&config, Status::NOT_FOUND, false);
    let text = String::from_utf8(response).expect("response should be text");

    assert!(text.contains("404 Not Found"));
    assert!(text.contains("custom not found"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn build_error_response_falls_back_when_error_page_cannot_be_read() {
    let mut config = ServerConfig::default();
    config
        .error_pages
        .insert(404, PathBuf::from("/tmp/does-not-exist-servex-404.html"));
    let response = build_error_response(&config, Status::NOT_FOUND, false);
    let text = String::from_utf8(response).expect("response should be text");

    assert!(text.contains("<h1>404 Not Found</h1>"));
}

#[test]
fn status_from_code_maps_redirects_and_unknown_values() {
    assert_eq!(status_from_code(301), Some(Status::MOVED_PERMANENTLY));
    assert_eq!(status_from_code(302), Some(Status::FOUND));
    assert_eq!(status_from_code(308), Some(Status::PERMANENT_REDIRECT));
    assert_eq!(status_from_code(999), None);
}

#[test]
fn detect_content_type_returns_expected_values() {
    assert_eq!(
        detect_content_type(Path::new("index.html")),
        "text/html; charset=utf-8"
    );
    assert_eq!(detect_content_type(Path::new("img.png")), "image/png");
    assert_eq!(
        detect_content_type(Path::new("unknown.blob")),
        "application/octet-stream"
    );
}
