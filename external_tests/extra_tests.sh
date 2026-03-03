#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

HOST="${HOST:-127.0.0.2}"
PORT="${PORT:-8080}"
BASE_URL="http://${HOST}:${PORT}"
APP_CONF="$ROOT_DIR/application.conf"
APP_CONF_BACKUP="$(mktemp)"
SERVER_LOG="/tmp/servex_extra_server.log"

cp "$APP_CONF" "$APP_CONF_BACKUP"

cleanup() {
  cp "$APP_CONF_BACKUP" "$APP_CONF" || true
  rm -f "$APP_CONF_BACKUP"
  if [[ -n "${SERVER_PID:-}" ]]; then
    kill "$SERVER_PID" >/dev/null 2>&1 || true
    wait "$SERVER_PID" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

require() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "Missing dependency: $1" >&2
    exit 1
  }
}

pass() { echo "[PASS] $1"; }
fail() { echo "[FAIL] $1" >&2; exit 1; }

require curl
require python3

check_port_available() {
  if command -v ss >/dev/null 2>&1; then
    if ss -ltn "( sport = :${PORT} )" | grep -q ":${PORT}"; then
      fail "Port ${PORT} is already in use on ${HOST}. Stop the running server first."
    fi
  fi
}

start_server() {
  check_port_available
  cargo run --quiet > "$SERVER_LOG" 2>&1 &
  SERVER_PID=$!
  for _ in $(seq 1 60); do
    if ! kill -0 "$SERVER_PID" >/dev/null 2>&1; then
      echo "Server exited before becoming ready. Log:" >&2
      sed -n '1,220p' "$SERVER_LOG" >&2
      return 1
    fi
    if curl -sS --noproxy '*' "${BASE_URL}/ok" >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.25
  done
  echo "Server not ready. Log:" >&2
  sed -n '1,220p' "$SERVER_LOG" >&2
  return 1
}

restart_server() {
  if [[ -n "${SERVER_PID:-}" ]]; then
    kill "$SERVER_PID" >/dev/null 2>&1 || true
    wait "$SERVER_PID" >/dev/null 2>&1 || true
    unset SERVER_PID
  fi
  start_server
}

status_code() {
  local method="$1"
  local url="$2"
  shift 2
  curl -sS -o /tmp/servex_extra_body.txt -w "%{http_code}" --noproxy '*' -X "$method" "$url" "$@"
}

echo "Running extra tests against ${BASE_URL}"
start_server

# 1) Session cookie issuance and reuse.
curl -sS -D /tmp/servex_extra_h1.txt --noproxy '*' "${BASE_URL}/ok" -o /tmp/servex_extra_b1.txt >/dev/null
cookie_line="$(tr -d '\r' < /tmp/servex_extra_h1.txt | sed -n 's/^Set-Cookie: //p' | head -n1)"
[[ "$cookie_line" == LOCALSERVER_SESSION=* ]] || fail "Expected Set-Cookie for first session request"

cookie_kv="${cookie_line%%;*}"
curl -sS -D /tmp/servex_extra_h2.txt --noproxy '*' -H "Cookie: ${cookie_kv}" "${BASE_URL}/ok" -o /tmp/servex_extra_b2.txt >/dev/null
second_cookie="$(tr -d '\r' < /tmp/servex_extra_h2.txt | sed -n 's/^Set-Cookie: //p' | head -n1)"
[[ -z "$second_cookie" ]] || fail "Expected no Set-Cookie when sending existing session cookie"
pass "Session cookie is issued and reused"

# 2) Session timeout (short timeout config -> old cookie should rotate).
awk '
BEGIN {in_session=0}
/^\[session\]/ {in_session=1; print; next}
/^\[/ {in_session=0; print; next}
{
  if (in_session && $0 ~ /^timeout[[:space:]]*=/) {
    print "timeout = 1";
  } else {
    print;
  }
}
' "$APP_CONF_BACKUP" > "$APP_CONF"

restart_server

curl -sS -D /tmp/servex_extra_h3.txt --noproxy '*' "${BASE_URL}/ok" -o /tmp/servex_extra_b3.txt >/dev/null
first_timeout_cookie="$(tr -d '\r' < /tmp/servex_extra_h3.txt | sed -n 's/^Set-Cookie: //p' | head -n1)"
first_timeout_kv="${first_timeout_cookie%%;*}"
sleep 2
curl -sS -D /tmp/servex_extra_h4.txt --noproxy '*' -H "Cookie: ${first_timeout_kv}" "${BASE_URL}/ok" -o /tmp/servex_extra_b4.txt >/dev/null
second_timeout_cookie="$(tr -d '\r' < /tmp/servex_extra_h4.txt | sed -n 's/^Set-Cookie: //p' | head -n1)"
[[ -n "$second_timeout_cookie" ]] || fail "Expected new Set-Cookie after session timeout expiry"
pass "Session timeout rotates cookie"

# Restore original config and restart for remaining tests.
cp "$APP_CONF_BACKUP" "$APP_CONF"
restart_server

# 3) HTTP/1.1 request without Host should be 400.
python3 - <<'PY' "$HOST" "$PORT"
import socket
import sys
host = sys.argv[1]
port = int(sys.argv[2])
s = socket.create_connection((host, port), timeout=2)
s.sendall(b"GET /ok HTTP/1.1\r\nConnection: close\r\n\r\n")
s.shutdown(socket.SHUT_WR)
resp = b""
while True:
    chunk = s.recv(4096)
    if not chunk:
        break
    resp += chunk
s.close()
if not resp.startswith(b"HTTP/1.1 400 Bad Request"):
    raise SystemExit("Expected 400 for HTTP/1.1 missing Host header")
PY
pass "HTTP/1.1 missing Host returns 400"

# 4) Path traversal should be blocked.
code="$(status_code GET "${BASE_URL}/static/../index.html" --path-as-is)"
[[ "$code" == "403" ]] || fail "Expected 403 for traversal path, got ${code}"
pass "Path traversal is blocked"

# 5) DELETE non-existing file should return 404.
code="$(status_code DELETE "${BASE_URL}/uploads/does_not_exist_foo.txt")"
[[ "$code" == "404" ]] || fail "Expected 404 on deleting non-existing file, got ${code}"
pass "DELETE non-existing file returns 404"

# 6) CGI env variables should be present.
cgi_env="$(curl -sS --noproxy '*' "${BASE_URL}/cgi-bin/env.py")"
[[ "$cgi_env" == *"REQUEST_METHOD=GET"* ]] || fail "CGI env missing REQUEST_METHOD"
[[ "$cgi_env" == *"PATH_INFO="* ]] || fail "CGI env missing PATH_INFO"
pass "CGI env variables are provided"

# 7) Custom error page body should be served.
curl -sS --noproxy '*' "${BASE_URL}/not-found-page" -o /tmp/servex_extra_404.html -w "%{http_code}" >/tmp/servex_extra_404_code.txt
code="$(cat /tmp/servex_extra_404_code.txt)"
[[ "$code" == "404" ]] || fail "Expected 404 status, got ${code}"
grep -q "404 Not Found" /tmp/servex_extra_404.html || fail "Custom 404 body not served"
pass "Custom error page is served"

echo "All extra tests passed."
