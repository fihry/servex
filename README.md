# LocalServer

A lightweight, HTTP/1.1-compliant web server written in Rust with non-blocking I/O.

## Features

- HTTP/1.1 compliant
- Non-blocking I/O using `mio`
- Static file serving
- CGI script execution
- File uploads
- Cookie and session management
- Configurable routes and error pages
- Multiple virtual servers

## Building
```bash
cargo build --release
```

## Running
```bash
cargo run --release
```

## Configuration

Edit `config.yaml` to configure servers, routes, and error pages.

## Testing
```bash
# Unit tests
cargo test

# Stress testing with siege
siege -b http://localhost:8080
```

## Architecture

See the architecture documentation in `/docs` for detailed information about the server design.

## License

Educational use only.
