use std::fs;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

struct TestServer {
    child: Child,
    dir: PathBuf,
    port: u16,
    clock_tick_file: Option<PathBuf>,
}

impl Drop for TestServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn unique_test_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("servex_{prefix}_{nanos}"))
}

fn find_free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind ephemeral port");
    listener
        .local_addr()
        .expect("missing local addr")
        .port()
}

fn write_config(dir: &Path, port: u16, global_timeout: u64, session: Option<&str>) {
    let root = dir.join("www");
    fs::create_dir_all(&root).expect("failed to create root");
    fs::write(root.join("ok"), "ok").expect("failed to create /ok resource");

    let mut content = format!(
        "[global]\nmax_body_size = 1048576\ntimeout = {global_timeout}\nkeep_alive = true\n\n\
[server:main]\nserver_name = localhost\nhost = 127.0.0.1\nports = {port}\nroot = ./www\n\n\
[route:main:ok]\npath = /ok\nmethods = GET\nindex = ok\nautoindex = false\n"
    );
    if let Some(session_block) = session {
        content.push('\n');
        content.push_str(session_block);
        content.push('\n');
    }
    fs::write(dir.join("application.conf"), content).expect("failed to write application.conf");
}

fn wait_until_ready(port: u16) {
    let mut attempts = 0;
    while attempts < 60 {
        if let Ok(response) = http_request(
            port,
            "GET /ok HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        ) {
            if response.starts_with("HTTP/1.1 200") {
                return;
            }
        }
        attempts += 1;
        sleep(Duration::from_millis(100));
    }
    panic!("server did not become ready");
}

fn start_server(
    name: &str,
    timeout: u64,
    session: Option<&str>,
    clock_step_ms: Option<u64>,
) -> TestServer {
    let dir = unique_test_dir(name);
    fs::create_dir_all(&dir).expect("failed to create test dir");
    let port = find_free_port();
    write_config(&dir, port, timeout, session);

    let bin = resolve_binary_path();
    let mut cmd = Command::new(bin);
    cmd.current_dir(&dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut clock_tick_file = None;
    if let Some(step_ms) = clock_step_ms {
        let tick_file = dir.join("clock_ticks");
        fs::write(&tick_file, "0\n").expect("failed to initialize clock tick file");
        cmd.env("SERVEX_CLOCK_MODE", "manual")
            .env("SERVEX_CLOCK_TICK_FILE", &tick_file)
            .env("SERVEX_CLOCK_STEP_MS", step_ms.to_string());
        clock_tick_file = Some(tick_file);
    }
    let child = cmd.spawn().expect("failed to start servex");

    let server = TestServer {
        child,
        dir,
        port,
        clock_tick_file,
    };
    wait_until_ready(server.port);
    server
}

fn set_clock_ticks(server: &TestServer, ticks: u64) {
    if let Some(path) = &server.clock_tick_file {
        fs::write(path, format!("{ticks}\n")).expect("failed to set clock ticks");
    } else {
        panic!("clock ticks requested for server without mock clock");
    }
}

fn resolve_binary_path() -> PathBuf {
    if let Ok(value) = std::env::var("CARGO_BIN_EXE_servex") {
        return PathBuf::from(value);
    }

    let current = std::env::current_exe().expect("cannot resolve current test executable path");
    let deps_dir = current.parent().expect("missing parent for test executable");
    let direct = deps_dir
        .parent()
        .expect("missing parent target/debug directory")
        .join("servex");
    if direct.exists() {
        return direct;
    }

    panic!("could not find servex binary path")
}

fn http_request(port: u16, request: &str) -> Result<String, String> {
    let mut stream =
        TcpStream::connect(("127.0.0.1", port)).map_err(|e| format!("connect failed: {e}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .map_err(|e| format!("set read timeout failed: {e}"))?;
    stream
        .write_all(request.as_bytes())
        .map_err(|e| format!("write failed: {e}"))?;
    let _ = stream.shutdown(Shutdown::Write);
    let mut data = Vec::new();
    stream
        .read_to_end(&mut data)
        .map_err(|e| format!("read failed: {e}"))?;
    String::from_utf8(data).map_err(|e| format!("invalid utf8 response: {e}"))
}

fn header_value<'a>(response: &'a str, header: &str) -> Option<&'a str> {
    let needle = format!("{}:", header.to_ascii_lowercase());
    response
        .split("\r\n")
        .find_map(|line| {
            let lower = line.to_ascii_lowercase();
            if lower.starts_with(&needle) {
                line.split_once(':').map(|(_, value)| value.trim())
            } else {
                None
            }
        })
}

#[test]
fn integration_connection_idle_timeout_returns_408() {
    let server = start_server("idle_timeout", 1, None, Some(1_500));
    let mut stream = TcpStream::connect(("127.0.0.1", server.port)).expect("connect failed");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("set read timeout failed");

    stream
        .write_all(b"GET /ok HTTP/1.1\r\nHost: localhost\r\n")
        .expect("partial request write failed");

    set_clock_ticks(&server, 2);

    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .expect("failed reading timeout response");
    let response_text = String::from_utf8(response).expect("invalid utf8 response");
    assert!(
        response_text.starts_with("HTTP/1.1 408 Request Timeout"),
        "expected 408 timeout response, got: {response_text}"
    );
}

#[test]
fn integration_session_disabled_does_not_send_cookie() {
    let server = start_server(
        "session_disabled",
        30,
        Some(
            "[session]\n\
enabled = false\n\
timeout = 60\n\
cookie_name = LOCALSERVER_SESSION\n\
secure = false\n\
http_only = true\n",
        ),
        None,
    );

    let response = http_request(
        server.port,
        "GET /ok HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
    )
    .expect("request failed");
    assert!(response.starts_with("HTTP/1.1 200"));
    assert!(header_value(&response, "Set-Cookie").is_none());
}

#[test]
fn integration_session_timeout_rotates_cookie() {
    let server = start_server(
        "session_timeout",
        30,
        Some(
            "[session]\n\
enabled = true\n\
timeout = 1\n\
cookie_name = LOCALSERVER_SESSION\n\
secure = false\n\
http_only = true\n",
        ),
        Some(1_500),
    );

    let first = http_request(
        server.port,
        "GET /ok HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
    )
    .expect("first request failed");
    let first_cookie = header_value(&first, "Set-Cookie")
        .expect("expected first Set-Cookie")
        .split(';')
        .next()
        .expect("missing cookie kv")
        .to_string();

    set_clock_ticks(&server, 2);

    let second = http_request(
        server.port,
        &format!(
            "GET /ok HTTP/1.1\r\nHost: localhost\r\nCookie: {first_cookie}\r\nConnection: close\r\n\r\n"
        ),
    )
    .expect("second request failed");
    let second_cookie = header_value(&second, "Set-Cookie").expect("expected rotated Set-Cookie");
    assert_ne!(
        second_cookie.split(';').next().unwrap_or(""),
        first_cookie,
        "session cookie should rotate after timeout"
    );
}

    
