#!/usr/bin/env bash
set -euo pipefail

HOST="${HOST:-127.0.0.2}"
PORT="${PORT:-8080}"
BASE_URL="http://${HOST}:${PORT}"

require() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "Missing dependency: $1" >&2
    exit 1
  }
}

require curl
require python3

fail() {
  echo "[FAIL] $1" >&2
  exit 1
}

pass() {
  echo "[PASS] $1"
}

status_code() {
  local method="$1"
  local path="$2"
  curl -sS -o /tmp/servex_ext_body.txt -w "%{http_code}" --noproxy '*' -X "$method" "${BASE_URL}${path}"
}

echo "Running external tests against ${BASE_URL}"

code="$(status_code GET /ok)"
[[ "$code" == "200" ]] || fail "GET /ok expected 200, got ${code}"
pass "GET /ok returns 200"

code="$(status_code PUT /x)"
[[ "$code" == "405" ]] || fail "PUT /x expected 405, got ${code}"
pass "PUT /x returns 405"

python3 - <<'PY' "$HOST" "$PORT"
import socket
import sys

host = sys.argv[1]
port = int(sys.argv[2])

s = socket.create_connection((host, port), timeout=2)
s.sendall(b"GET / HTTP/1.1\r\nHost localhost\r\nConnection: close\r\n\r\n")
s.shutdown(socket.SHUT_WR)
response = b""
while True:
    chunk = s.recv(4096)
    if not chunk:
        break
    response += chunk
s.close()

if not response.startswith(b"HTTP/1.1 400 Bad Request"):
    sys.stderr.write("Expected malformed request to return 400\n")
    sys.exit(1)
PY
pass "Malformed header returns 400"

python3 - <<'PY' "$HOST" "$PORT"
import socket
import sys

host = sys.argv[1]
port = int(sys.argv[2])

request = (
    b"GET /one HTTP/1.1\r\nHost: localhost\r\n\r\n"
    b"GET /two HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
)

s = socket.create_connection((host, port), timeout=2)
s.sendall(request)
s.shutdown(socket.SHUT_WR)
response = b""
while True:
    chunk = s.recv(4096)
    if not chunk:
        break
    response += chunk
s.close()

status_lines = response.count(b"HTTP/1.1 ")
if status_lines < 2:
    sys.stderr.write(f"Expected 2 pipelined responses, got {status_lines}\n")
    sys.exit(1)
PY
pass "Pipelined requests return two responses"

echo "All external tests passed."
