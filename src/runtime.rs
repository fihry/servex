use std::collections::HashMap;
use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::net::SocketAddr;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use mio::net::{TcpListener, TcpStream};
use mio::{Events, Interest, Poll, Token};

use crate::config::models::{CgiConfig, ServerConfig};
use crate::http::builder::response::ResponseBuilder;
use crate::http::models::method::Method;
use crate::http::models::status::Status;
use crate::http::parser::chunked::ChunkedError;
use crate::http::parser::multipart::parse_multipart;
use crate::http::parser::request::{parse_request, RequestParseError};
use crate::routing::{RouteDecision, Router};

const READ_BUFFER_SIZE: usize = 8 * 1024;

struct Connection {
    socket: TcpStream,
    server_candidates: Vec<usize>,
    read_buf: Vec<u8>,
    write_buf: Vec<u8>,
    close_after_write: bool,
    peer_closed: bool,
    last_active: Instant,
}

struct ListenerEntry {
    listener: TcpListener,
    server_candidates: Vec<usize>,
}

impl Connection {
    fn new(socket: TcpStream, server_candidates: Vec<usize>) -> Self {
        Self {
            socket,
            server_candidates,
            read_buf: Vec::new(),
            write_buf: Vec::new(),
            close_after_write: false,
            peer_closed: false,
            last_active: Instant::now(),
        }
    }
}

pub fn run(config: ServerConfig) -> Result<(), String> {
    let router = Router::new(&config)?;
    let mut poll = Poll::new().map_err(|e| format!("failed to create poll: {}", e))?;
    let mut events = Events::with_capacity(1024);

    let mut listeners = HashMap::<Token, ListenerEntry>::new();
    let mut connections = HashMap::<Token, Connection>::new();
    let mut next_token = 0usize;
    let mut sessions = HashMap::<String, Instant>::new();

    let mut listeners_by_addr: HashMap<SocketAddr, Vec<usize>> = HashMap::new();
    for (server_index, server) in config.servers.iter().enumerate() {
        for port in &server.ports {
            let addr: SocketAddr = format!("{}:{}", server.host, port)
                .parse()
                .map_err(|e| format!("invalid listener addr for {}:{}: {}", server.host, port, e))?;
            listeners_by_addr
                .entry(addr)
                .or_insert_with(Vec::new)
                .push(server_index);
        }
    }

    for (addr, server_candidates) in listeners_by_addr {
        let mut listener =
            TcpListener::bind(addr).map_err(|e| format!("failed to bind {}: {}", addr, e))?;
        let token = Token(next_token);
        next_token += 1;
        poll.registry()
            .register(&mut listener, token, Interest::READABLE)
            .map_err(|e| format!("failed to register listener {}: {}", addr, e))?;
        listeners.insert(
            token,
            ListenerEntry {
                listener,
                server_candidates,
            },
        );
    }

    loop {
        poll.poll(&mut events, Some(Duration::from_millis(200)))
            .map_err(|e| format!("poll failed: {}", e))?;

        for event in events.iter() {
            let token = event.token();
            if listeners.contains_key(&token) {
                accept_clients(
                    &mut listeners,
                    &mut connections,
                    poll.registry(),
                    token,
                    &mut next_token,
                )?;
                continue;
            }

            if let Some(conn) = connections.get_mut(&token) {
                if event.is_readable() {
                    read_from_client(conn, &config, &router, &mut sessions);
                }
                if event.is_writable() {
                    write_to_client(conn);
                }
            }
        }

        let mut to_remove = Vec::new();
        let now = Instant::now();
        for (token, conn) in connections.iter_mut() {
            if now.duration_since(conn.last_active).as_secs() > config.global.timeout {
                if conn.write_buf.is_empty() {
                    conn.write_buf = build_error_response(&config, Status::REQUEST_TIMEOUT, false);
                }
                conn.close_after_write = true;
            }

            if conn.peer_closed && conn.write_buf.is_empty() {
                to_remove.push(*token);
                continue;
            }

            if conn.close_after_write && conn.write_buf.is_empty() {
                to_remove.push(*token);
                continue;
            }

            let interest = if conn.write_buf.is_empty() {
                Interest::READABLE
            } else {
                Interest::READABLE.add(Interest::WRITABLE)
            };
            if let Err(e) = poll
                .registry()
                .reregister(&mut conn.socket, *token, interest)
            {
                eprintln!("reregister failed for {:?}: {}", token, e);
                to_remove.push(*token);
            }
        }

        for token in to_remove {
            if let Some(mut conn) = connections.remove(&token) {
                let _ = poll.registry().deregister(&mut conn.socket);
            }
        }
    }
}

fn accept_clients(
    listeners: &mut HashMap<Token, ListenerEntry>,
    connections: &mut HashMap<Token, Connection>,
    registry: &mio::Registry,
    listener_token: Token,
    next_token: &mut usize,
) -> Result<(), String> {
    let entry = listeners
        .get_mut(&listener_token)
        .ok_or_else(|| "missing listener for token".to_string())?;

    loop {
        match entry.listener.accept() {
            Ok((mut stream, _)) => {
                let token = Token(*next_token);
                *next_token += 1;
                registry
                    .register(&mut stream, token, Interest::READABLE)
                    .map_err(|e| format!("failed to register connection: {}", e))?;
                connections.insert(
                    token,
                    Connection::new(stream, entry.server_candidates.clone()),
                );
            }
            Err(err) if err.kind() == ErrorKind::WouldBlock => break,
            Err(err) => {
                eprintln!("accept failed: {}", err);
                break;
            }
        }
    }
    Ok(())
}

fn read_from_client(
    conn: &mut Connection,
    config: &ServerConfig,
    router: &Router,
    sessions: &mut HashMap<String, Instant>,
) {
    let mut buffer = [0u8; READ_BUFFER_SIZE];
    match conn.socket.read(&mut buffer) {
        Ok(0) => {
            conn.peer_closed = true;
        }
        Ok(n) => {
            conn.last_active = Instant::now();
            conn.read_buf.extend_from_slice(&buffer[..n]);
        }
        Err(err) if err.kind() == ErrorKind::WouldBlock => {}
        Err(_) => {
            conn.close_after_write = true;
        }
    }

    loop {
        match parse_request(&conn.read_buf) {
            Ok((request, consumed)) => {
                conn.read_buf.drain(..consumed);
                let keep_alive = should_keep_alive(config, request.version.as_str(), &request);
                let response = build_response(
                    config,
                    router,
                    &conn.server_candidates,
                    sessions,
                    &request,
                    keep_alive,
                );
                conn.write_buf.extend_from_slice(&response);
                if !keep_alive {
                    conn.close_after_write = true;
                }
            }
            Err(RequestParseError::Incomplete) => break,
            Err(RequestParseError::Chunked(ChunkedError::Incomplete)) => break,
            Err(_) => {
                let response = build_error_response(config, Status::BAD_REQUEST, false);
                conn.write_buf.extend_from_slice(&response);
                conn.close_after_write = true;
                conn.read_buf.clear();
                break;
            }
        }
    }
}

fn write_to_client(conn: &mut Connection) {
    if conn.write_buf.is_empty() {
        return;
    }
    match conn.socket.write(&conn.write_buf) {
        Ok(0) => {
            conn.close_after_write = true;
        }
        Ok(n) => {
            conn.last_active = Instant::now();
            conn.write_buf.drain(..n);
        }
        Err(err) if err.kind() == ErrorKind::WouldBlock => {}
        Err(_) => {
            conn.close_after_write = true;
        }
    }
}

fn should_keep_alive(
    config: &ServerConfig,
    version: &str,
    request: &crate::http::models::request::Request,
) -> bool {
    if !config.global.keep_alive {
        return false;
    }
    if let Some(value) = request.headers.get("connection") {
        if value.eq_ignore_ascii_case("close") {
            return false;
        }
        if version == "HTTP/1.0" && !value.eq_ignore_ascii_case("keep-alive") {
            return false;
        }
    } else if version == "HTTP/1.0" {
        return false;
    }
    true
}

fn build_response(
    config: &ServerConfig,
    router: &Router,
    server_candidates: &[usize],
    sessions: &mut HashMap<String, Instant>,
    request: &crate::http::models::request::Request,
    keep_alive: bool,
) -> Vec<u8> {
    if request.version == "HTTP/1.1" && request.headers.get("host").is_none() {
        return build_error_response(config, Status::BAD_REQUEST, keep_alive);
    }

    if request.body.len() > config.global.max_body_size {
        return build_error_response(config, Status::PAYLOAD_TOO_LARGE, keep_alive);
    }

    let clean_path = strip_query(&request.path);
    let server = match router.select_server(server_candidates, request.headers.get("host")) {
        Some(server) => server,
        None => return build_error_response(config, Status::INTERNAL_SERVER_ERROR, keep_alive),
    };

    match router.resolve(server, clean_path, &request.method) {
        RouteDecision::NotFound => build_error_response(config, Status::NOT_FOUND, keep_alive),
        RouteDecision::MethodNotAllowed => {
            build_error_response(config, Status::METHOD_NOT_ALLOWED, keep_alive)
        }
        RouteDecision::Redirect { status, target } => {
            let status = status_from_code(status).unwrap_or(Status::FOUND);
            let mut extra_headers = vec![("Location".to_string(), target)];
            attach_session_cookie(config, sessions, request, &mut extra_headers);
            build_bytes(
                request.version.as_str(),
                status,
                "text/plain; charset=utf-8",
                Vec::new(),
                keep_alive,
                extra_headers,
            )
        }
        RouteDecision::Matched {
            route_path,
            request_path,
            root,
            index,
            autoindex,
            upload_dir,
            cgi,
            max_file_size,
            ..
        } => handle_matched(
            config,
            sessions,
            request,
            keep_alive,
            route_path.as_str(),
            request_path.as_str(),
            root.as_path(),
            index.as_deref(),
            autoindex,
            upload_dir.as_deref(),
            cgi.as_ref(),
            max_file_size,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_matched(
    config: &ServerConfig,
    sessions: &mut HashMap<String, Instant>,
    request: &crate::http::models::request::Request,
    keep_alive: bool,
    route_path: &str,
    request_path: &str,
    root: &Path,
    index: Option<&str>,
    autoindex: bool,
    upload_dir: Option<&Path>,
    cgi: Option<&CgiConfig>,
    max_file_size: Option<usize>,
) -> Vec<u8> {
    if let Some(limit) = max_file_size {
        if request.body.len() > limit {
            return build_error_response(config, Status::PAYLOAD_TOO_LARGE, keep_alive);
        }
    }

    let mut extra_headers = Vec::new();
    attach_session_cookie(config, sessions, request, &mut extra_headers);

    match &request.method {
        Method::Get => {
            let effective_root = upload_dir.unwrap_or(root);
            if let Some(cgi_cfg) = cgi {
                if request_path.ends_with(&cgi_cfg.extension) {
                    return execute_cgi(
                        request.version.as_str(),
                        request,
                        cgi_cfg,
                        effective_root,
                        route_path,
                        keep_alive,
                        extra_headers,
                        config,
                    );
                }
            }
            match resolve_target_path(effective_root, route_path, request_path, index, autoindex) {
                Ok(Target::File(path)) => serve_file(
                    request.version.as_str(),
                    path.as_path(),
                    keep_alive,
                    extra_headers,
                    config,
                ),
                Ok(Target::DirectoryListing(path)) => {
                    let body = directory_listing(path.as_path(), request_path);
                    build_bytes(
                        request.version.as_str(),
                        Status::OK,
                        "text/html; charset=utf-8",
                        body.into_bytes(),
                        keep_alive,
                        extra_headers,
                    )
                }
                Err(status) => build_error_response(config, status, keep_alive),
            }
        }
        Method::Post => {
            if let Some(cgi_cfg) = cgi {
                if request_path.ends_with(&cgi_cfg.extension) {
                    return execute_cgi(
                        request.version.as_str(),
                        request,
                        cgi_cfg,
                        root,
                        route_path,
                        keep_alive,
                        extra_headers,
                        config,
                    );
                }
            }

            let upload_dir = upload_dir.unwrap_or(root);
            match store_upload(upload_dir, request) {
                Ok(location) => {
                    let body = format!("uploaded: {}\n", location.display()).into_bytes();
                    let headers = vec![("Location".to_string(), location.display().to_string())];
                    build_bytes(
                        request.version.as_str(),
                        Status::CREATED,
                        "text/plain; charset=utf-8",
                        body,
                        keep_alive,
                        [extra_headers, headers].concat(),
                    )
                }
                Err(status) => build_error_response(config, status, keep_alive),
            }
        }
        Method::Delete => {
            let effective_root = upload_dir.unwrap_or(root);
            match resolve_delete_target(effective_root, route_path, request_path) {
            Ok(path) => match fs::remove_file(&path) {
                Ok(_) => build_bytes(
                    request.version.as_str(),
                    Status::NO_CONTENT,
                    "text/plain; charset=utf-8",
                    Vec::new(),
                    keep_alive,
                    extra_headers,
                ),
                Err(_) => build_error_response(config, Status::NOT_FOUND, keep_alive),
            },
            Err(status) => build_error_response(config, status, keep_alive),
        }
        }
        _ => build_error_response(config, Status::METHOD_NOT_ALLOWED, keep_alive),
    }
}

enum Target {
    File(PathBuf),
    DirectoryListing(PathBuf),
}

fn resolve_target_path(
    root: &Path,
    route_path: &str,
    request_path: &str,
    index: Option<&str>,
    autoindex: bool,
) -> Result<Target, Status> {
    let mut path = resolve_relative_path(root, route_path, request_path)?;

    if path.is_dir() {
        if let Some(index_name) = index {
            path = path.join(index_name);
            if path.exists() && path.is_file() {
                return Ok(Target::File(path));
            }
        }
        if autoindex {
            return Ok(Target::DirectoryListing(path));
        }
        return Err(Status::FORBIDDEN);
    }

    if !path.exists() || !path.is_file() {
        return Err(Status::NOT_FOUND);
    }
    Ok(Target::File(path))
}

fn resolve_delete_target(
    root: &Path,
    route_path: &str,
    request_path: &str,
) -> Result<PathBuf, Status> {
    let path = resolve_relative_path(root, route_path, request_path)?;
    if !path.exists() || !path.is_file() {
        return Err(Status::NOT_FOUND);
    }
    Ok(path)
}

fn resolve_relative_path(
    root: &Path,
    route_path: &str,
    request_path: &str,
) -> Result<PathBuf, Status> {
    let relative = if route_path == "/" {
        request_path.to_string()
    } else {
        request_path
            .strip_prefix(route_path)
            .unwrap_or(request_path)
            .to_string()
    };
    let relative = relative.trim_start_matches('/');
    let relative_path = Path::new(relative);
    if !is_safe_relative_path(relative_path) {
        return Err(Status::FORBIDDEN);
    }
    Ok(root.join(relative_path))
}

fn is_safe_relative_path(path: &Path) -> bool {
    !path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
}

fn serve_file(
    version: &str,
    path: &Path,
    keep_alive: bool,
    extra_headers: Vec<(String, String)>,
    config: &ServerConfig,
) -> Vec<u8> {
    match fs::read(path) {
        Ok(body) => build_bytes(
            version,
            Status::OK,
            detect_content_type(path),
            body,
            keep_alive,
            extra_headers,
        ),
        Err(_) => build_error_response(config, Status::NOT_FOUND, keep_alive),
    }
}

fn execute_cgi(
    version: &str,
    request: &crate::http::models::request::Request,
    cgi: &CgiConfig,
    root: &Path,
    route_path: &str,
    keep_alive: bool,
    extra_headers: Vec<(String, String)>,
    config: &ServerConfig,
) -> Vec<u8> {
    let script_path = match resolve_relative_path(root, route_path, strip_query(&request.path)) {
        Ok(path) => path,
        Err(status) => return build_error_response(config, status, keep_alive),
    };

    let mut command = Command::new(&cgi.executor);
    command
        .arg(&script_path)
        .env("PATH_INFO", script_path.display().to_string())
        .env("REQUEST_METHOD", request.method.to_string())
        .env("CONTENT_TYPE", request.headers.get("content-type").unwrap_or(""))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(_) => return build_error_response(config, Status::INTERNAL_SERVER_ERROR, keep_alive),
    };

    if let Some(stdin) = child.stdin.as_mut() {
        let _ = stdin.write_all(&request.body);
    }

    match child.wait_with_output() {
        Ok(output) => {
            if !output.status.success() {
                return build_error_response(config, Status::INTERNAL_SERVER_ERROR, keep_alive);
            }
            build_bytes(
                version,
                Status::OK,
                "text/plain; charset=utf-8",
                output.stdout,
                keep_alive,
                extra_headers,
            )
        }
        Err(_) => build_error_response(config, Status::INTERNAL_SERVER_ERROR, keep_alive),
    }
}

fn store_upload(
    upload_dir: &Path,
    request: &crate::http::models::request::Request,
) -> Result<PathBuf, Status> {
    if !upload_dir.exists() {
        return Err(Status::NOT_FOUND);
    }
    if !upload_dir.is_dir() {
        return Err(Status::FORBIDDEN);
    }

    let mut saved_first = None;

    if let Some(content_type) = request.headers.get("content-type") {
        if content_type.contains("multipart/form-data") {
            if let Some(boundary) = extract_boundary(content_type) {
                let parts =
                    parse_multipart(&request.body, boundary).map_err(|_| Status::BAD_REQUEST)?;
                for (idx, part) in parts.iter().enumerate() {
                    let filename = part
                        .headers
                        .get("content-disposition")
                        .and_then(extract_filename)
                        .unwrap_or_else(|| format!("upload_part_{}.bin", idx));
                    let safe_name = sanitize_filename(&filename);
                    let path = upload_dir.join(safe_name);
                    fs::write(&path, &part.data).map_err(|_| Status::INTERNAL_SERVER_ERROR)?;
                    if saved_first.is_none() {
                        saved_first = Some(path);
                    }
                }
                return saved_first.ok_or(Status::BAD_REQUEST);
            }
        }
    }

    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let path = upload_dir.join(format!("upload_{}.bin", millis));
    fs::write(&path, &request.body).map_err(|_| Status::INTERNAL_SERVER_ERROR)?;
    Ok(path)
}

fn extract_boundary(content_type: &str) -> Option<&str> {
    content_type.split(';').find_map(|segment| {
        let segment = segment.trim();
        segment.strip_prefix("boundary=")
    })
}

fn extract_filename(content_disposition: &str) -> Option<String> {
    content_disposition
        .split(';')
        .find_map(|segment| segment.trim().strip_prefix("filename="))
        .map(|name| name.trim_matches('"').to_string())
}

fn sanitize_filename(name: &str) -> String {
    let fallback = "upload.bin".to_string();
    let file_name = Path::new(name).file_name().and_then(|v| v.to_str());
    match file_name {
        Some(value) if !value.trim().is_empty() => value.to_string(),
        _ => fallback,
    }
}

fn strip_query(path: &str) -> &str {
    path.split('?').next().unwrap_or(path)
}

fn attach_session_cookie(
    config: &ServerConfig,
    sessions: &mut HashMap<String, Instant>,
    request: &crate::http::models::request::Request,
    headers: &mut Vec<(String, String)>,
) {
    if !config.sessions.enabled() {
        return;
    }

    let now = Instant::now();
    sessions.retain(|_, created| {
        now.duration_since(*created).as_secs() <= config.sessions.timeout() as u64
    });

    let cookie_name = config.sessions.cookie_name();
    let existing = request.headers.get("cookie").and_then(|cookies| {
        cookies.split(';').find_map(|part| {
            let part = part.trim();
            let expected_prefix = format!("{}=", cookie_name);
            part.strip_prefix(expected_prefix.as_str())
                .map(|value| value.to_string())
        })
    });

    let session_id = match existing {
        Some(id) => id,
        None => generate_session_id(),
    };
    sessions.insert(session_id.clone(), now);

    if request.headers.get("cookie").is_none() {
        let mut value = format!("{}={}; Path=/; SameSite=Lax", cookie_name, session_id);
        if config.sessions.http_only() {
            value.push_str("; HttpOnly");
        }
        if config.sessions.secure() {
            value.push_str("; Secure");
        }
        headers.push(("Set-Cookie".to_string(), value));
    }
}

fn generate_session_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{:x}", nanos)
}

fn directory_listing(path: &Path, request_path: &str) -> String {
    let mut html = format!("<html><body><h1>Index of {}</h1><ul>", request_path);
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let href = if request_path.ends_with('/') {
                format!("{}{}", request_path, name)
            } else if request_path == "/" {
                format!("/{}", name)
            } else {
                format!("{}/{}", request_path, name)
            };
            html.push_str(&format!("<li><a href=\"{}\">{}</a></li>", href, name));
        }
    }
    html.push_str("</ul></body></html>");
    html
}

fn build_error_response(config: &ServerConfig, status: Status, keep_alive: bool) -> Vec<u8> {
    let body = match config.error_pages.get(&status.code) {
        Some(path) => fs::read(path).unwrap_or_else(|_| fallback_error_body(status)),
        None => fallback_error_body(status),
    };
    build_bytes(
        "HTTP/1.1",
        status,
        "text/html; charset=utf-8",
        body,
        keep_alive,
        Vec::new(),
    )
}

fn fallback_error_body(status: Status) -> Vec<u8> {
    format!(
        "<html><body><h1>{} {}</h1></body></html>",
        status.code, status.reason
    )
    .into_bytes()
}

fn build_bytes(
    version: &str,
    status: Status,
    content_type: &str,
    body: Vec<u8>,
    keep_alive: bool,
    extra_headers: Vec<(String, String)>,
) -> Vec<u8> {
    let builder = ResponseBuilder::new(
        version.to_string(),
        format!("{} {}", status.code, status.reason),
        content_type.to_string(),
        body,
        keep_alive,
        extra_headers,
    );
    builder.build()
}

fn status_from_code(code: u16) -> Option<Status> {
    match code {
        301 => Some(Status::MOVED_PERMANENTLY),
        302 => Some(Status::FOUND),
        303 => Some(Status::SEE_OTHER),
        307 => Some(Status::TEMPORARY_REDIRECT),
        308 => Some(Status::PERMANENT_REDIRECT),
        _ => Status::from_code(code),
    }
}

fn detect_content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or_default()
    {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "application/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "txt" => "text/plain; charset=utf-8",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}
