# Servex Audit Notes

## Core architecture
- Single process / single thread event loop based on `mio::Poll` in `src/runtime.rs`.
- One poll instance drives all listener and client sockets.
- Non-blocking sockets are only read/written when readiness events are emitted by poll.

## I/O multiplexing
- Multiplexing primitive: `mio::Poll` (`epoll` backend on Linux).
- Event wait: `poll.poll(&mut events, ...)`.
- Event dispatch: listener tokens accept clients, client tokens read or write.

## One poll and socket lifecycle
- Exactly one `Poll` is created in `run`.
- Listener sockets for all configured `(host, port)` are registered in that single poll.
- Connections are registered and re-registered on the same poll with readable/writable interests.
- Read/write syscalls check return values and handle `WouldBlock`.
- On fatal socket errors, connection is closed and deregistered.

## Hostname virtual servers
- Config supports multiple server sections (`[server:name]`) and hostname list via `server_name`.
- Runtime binds once per unique `(host, port)` and maps the listener to candidate server blocks.
- Request `Host` header selects the matched virtual server; fallback is first candidate.

## Routes and methods
- Longest-prefix route matching.
- Method allowlist enforced per route (`405` when disallowed).
- Redirect routes return configured redirect status and `Location`.

## Request handling
- Supports `Content-Length` and chunked transfer decoding.
- Returns `400` for malformed requests and invalid HTTP/1.1 missing Host header.
- Global body limit and route-level upload file-size limit enforced (`413`).

## Static files / directory index / uploads
- Static file serving with extension-based content types.
- Directory listing when `autoindex = true`.
- Directory index support via route `index`.
- Uploads support raw and multipart forms.
- Uploaded files are retrievable and deletable.

## CGI
- CGI executes configured extension via `std::process::Command`.
- Script path passed as first argument.
- `PATH_INFO`, `REQUEST_METHOD`, and `CONTENT_TYPE` are set in environment.
- Works for chunked and unchunked POST bodies.

## Sessions and cookies
- Basic in-memory session map with timeout cleanup.
- Cookie emitted via `Set-Cookie` when session is not already present.

## Test scripts
- Baseline checks: `external_tests/run.sh`.
- Audit-oriented checks: `external_tests/audit_run.sh`.
- Multi-server sample config: `audit/application.audit.conf`.
