# Servex

Servex is an HTTP/1.1 server written in Rust using non-blocking I/O with `mio`.
It serves static files, supports uploads, executes CGI scripts, and supports virtual hosts through a config file.

## Highlights

- Event-driven runtime (`mio::Poll`) with one process/thread.
- HTTP request parsing (including `Content-Length` and `Transfer-Encoding: chunked`).
- Route handling for `GET`, `POST`, `DELETE`.
- Static files, directory autoindex, redirects.
- Multipart and raw-body uploads.
- CGI execution via `std::process::Command` (e.g. `.py` scripts).
- Session cookie support with configurable timeout.
- Multiple server blocks and hostname-based virtual host selection on shared ports.
- Custom error pages (`400`, `403`, `404`, `405`, `413`, `500`).

## Project Layout

- `src/main.rs`: boot sequence (`load -> validate -> run`).
- `src/config/`: INI parsing, loading, validation, models.
- `src/runtime/`: event loop, connection lifecycle, response generation.
- `src/routing/`: route resolution and virtual-host server selection.
- `src/http/`: request parsers and response builder.
- `tests/integration_test.rs`: process/socket end-to-end integration tests.
- `external_tests/`: black-box external/audit test scripts.
- `application.conf`: default local config.
- `audit/application.audit.conf`: audit-focused config.

## Build

```bash
cargo build --release
```

## Run

Servex expects `application.conf` in the current working directory.

```bash
cargo run --release
```

Debug builds print parsed config at startup (`#[cfg(debug_assertions)]`).

## Configuration Format

Config file is INI-style (not YAML), for example:

- `[global]`:
  - `max_body_size` (bytes)
  - `timeout` (seconds)
  - `keep_alive` (`true/false`)
- `[error_pages]`: `status_code = path`
- `[server]` or `[server:<name>]`:
  - `server_name` (comma-separated hostnames)
  - `host`
  - `ports` (comma-separated)
  - `root`
- `[route:<server_name>:<route_name>]`:
  - `path`, `methods`
  - optional: `root`, `index`, `autoindex`
  - optional redirects: `redirect_status`, `redirect_target`
  - optional uploads: `upload_dir`, `max_file_size`
  - optional CGI: `cgi_extension`, `cgi_executor`
- `[session]`:
  - `enabled`, `timeout`, `cookie_name`, `secure`, `http_only`

Notes:

- Route `path` must start with `/`.
- When multiple server blocks share the same `host:port`, Host header selects virtual host.
- In single-server configs, routes referencing another server name are attached to that one server as fallback.

## Request Flow

1. Poll for events from listeners and client sockets.
2. Accept new clients on readable listeners.
3. Read client bytes, parse one or more requests from buffer.
4. Choose server block (listener candidates + `Host` header).
5. Resolve route and method behavior.
6. Queue response bytes.
7. Write responses on writable events.
8. Apply keep-alive and idle-timeout connection management.

## Testing

### Test Types Included

- Unit tests:
  - Located inside source modules under `src/` (`#[test]`).
  - Validate parser logic, config loading/validation, routing decisions, runtime helper behavior, and HTTP builder/parsers.
- Integration tests:
  - Located in `tests/integration_test.rs`.
  - Spawn the real `servex` binary and verify end-to-end behavior through TCP sockets.
- External black-box tests:
  - Located in `external_tests/`.
  - Execute shell-level checks against a running server (`run.sh`, `audit_run.sh`, `extra_tests.sh`).
- Stress/performance checks:
  - Manual load testing via `siege` for availability/performance validation.

### Rust tests

```bash
# unit + integration tests (Rust test harness)
cargo test

# binary-target tests (useful in restricted environments)
cargo test --bin servex

# integration tests only
cargo test --test integration_test
```

### External scripts

```bash
# basic external checks
./external_tests/run.sh

# full audit checks (uploads, CGI, redirects, virtual hosts, cookies)
./external_tests/audit_run.sh

# additional behavior checks
./external_tests/extra_tests.sh
```

`audit_run.sh` now fails early if port `8080` is already in use.

### Stress test

```bash
siege -b http://127.0.0.1:8080/ok
```

## Known Behaviors / Limits

- `HEAD`, `PUT`, etc. are parsed but not implemented as handlers (typically `405`).
- Session storage is in-memory (not persistent across restart).
- CGI output is treated as response body with `200 OK` when script exits successfully.

## License

Educational use.
