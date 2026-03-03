#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

APP_CONF="$ROOT_DIR/application.conf"
AUDIT_CONF="$ROOT_DIR/audit/application.audit.conf"
BACKUP_CONF="$(mktemp)"
SERVER_LOG="/tmp/servex_audit_server.log"
TEST_BODY="/tmp/servex_audit_body.txt"

cp "$APP_CONF" "$BACKUP_CONF"
restore() {
  cp "$BACKUP_CONF" "$APP_CONF" || true
  rm -f "$BACKUP_CONF"
}
trap restore EXIT

cp "$AUDIT_CONF" "$APP_CONF"

fail() {
  echo "[FAIL] $1" >&2
  exit 1
}

pass() {
  echo "[PASS] $1"
}

require() {
  command -v "$1" >/dev/null 2>&1 || fail "Missing dependency: $1"
}

require curl
require python3

cargo run --quiet > "$SERVER_LOG" 2>&1 &
SERVER_PID=$!
trap 'kill "$SERVER_PID" >/dev/null 2>&1 || true; restore' EXIT

ready=0
for _ in $(seq 1 40); do
  if curl -sS --noproxy '*' http://127.0.0.1:8080/ >/dev/null 2>&1; then
    ready=1
    break
  fi
  sleep 0.25
done
[[ "$ready" -eq 1 ]] || fail "Server failed to start (see $SERVER_LOG)"

code="$(curl -sS -o "$TEST_BODY" -w "%{http_code}" --noproxy '*' http://127.0.0.1:8080/ok)"
[[ "$code" == "200" ]] || fail "GET /ok expected 200 got $code"
pass "GET works"

code="$(curl -sS -o "$TEST_BODY" -w "%{http_code}" --noproxy '*' -X POST http://127.0.0.1:8080/uploads --data-binary 'audit-upload')"
[[ "$code" == "201" ]] || fail "POST /uploads expected 201 got $code"
upload_path="$(tr -d '\r' < "$TEST_BODY" | sed -n 's/^uploaded: //p' | head -n1)"
[[ -n "$upload_path" ]] || fail "Upload response did not return path"
pass "POST upload works"

upload_file="$(basename "$upload_path")"
code="$(curl -sS -o "$TEST_BODY" -w "%{http_code}" --noproxy '*' "http://127.0.0.1:8080/uploads/${upload_file}")"
[[ "$code" == "200" ]] || fail "GET uploaded file expected 200 got $code"
[[ "$(cat "$TEST_BODY")" == "audit-upload" ]] || fail "Uploaded file content mismatch"
pass "Uploaded file is retrievable and uncorrupted"

code="$(curl -sS -o "$TEST_BODY" -w "%{http_code}" --noproxy '*' -X DELETE "http://127.0.0.1:8080/uploads/${upload_file}")"
[[ "$code" == "204" ]] || fail "DELETE uploaded file expected 204 got $code"
pass "DELETE works"

code="$(curl -sS -o "$TEST_BODY" -w "%{http_code}" --noproxy '*' -X PUT http://127.0.0.1:8080/ok)"
[[ "$code" == "405" ]] || fail "Method restriction expected 405 got $code"
pass "Method restriction works"

code="$(
  python3 - <<'PY' | curl -sS -o "$TEST_BODY" -w "%{http_code}" --noproxy '*' -X POST http://127.0.0.1:8080/uploads --data-binary @-
import sys
sys.stdout.write('x' * (1024 * 1024 + 1))
PY
)"
[[ "$code" == "413" ]] || fail "Body size limit expected 413 got $code"
pass "Body size limit works"

code="$(curl -sS -o "$TEST_BODY" -w "%{http_code}" --noproxy '*' http://127.0.0.1:8080/does-not-exist)"
[[ "$code" == "404" ]] || fail "Wrong URL expected 404 got $code"
pass "404 works"

code="$(curl -sS -o "$TEST_BODY" -w "%{http_code}" --noproxy '*' http://127.0.0.1:8080/uploads/)"
[[ "$code" == "200" ]] || fail "Directory listing expected 200 got $code"
pass "Directory listing works"

location="$(curl -sS -D - --noproxy '*' http://127.0.0.1:8080/temp -o /dev/null | tr -d '\r' | sed -n 's/^Location: //p' | head -n1)"
[[ "$location" == "https://example.com" ]] || fail "Redirect Location mismatch"
pass "Redirect works"

cgi_output="$(printf 'hello-cgi' | curl -sS --noproxy '*' -X POST http://127.0.0.1:8080/cgi-bin/echo.py --data-binary @-)"
[[ "$cgi_output" == *"CGI ECHO"* ]] || fail "CGI output missing marker"
[[ "$cgi_output" == *"hello-cgi"* ]] || fail "CGI did not receive unchunked body"
pass "CGI with unchunked body works"

chunked_output="$(printf 'chunked-cgi' | curl -sS --noproxy '*' -H 'Transfer-Encoding: chunked' -X POST http://127.0.0.1:8080/cgi-bin/echo.py --data-binary @-)"
[[ "$chunked_output" == *"chunked-cgi"* ]] || fail "CGI did not receive chunked body"
pass "CGI with chunked body works"

main_body="$(curl -sS --noproxy '*' --resolve test.com:8080:127.0.0.1 http://test.com:8080/)"
[[ "$main_body" == *"Servex Home"* ]] || fail "Hostname response mismatch"
pass "Hostname routing works for single server"

cookie_header="$(curl -sS -D - --noproxy '*' http://127.0.0.1:8080/ok -o /dev/null | tr -d '\r' | sed -n 's/^Set-Cookie: //p' | head -n1)"
[[ "$cookie_header" == *"LOCALSERVER_SESSION="* ]] || fail "Session cookie missing"
pass "Session cookie is set"

kill "$SERVER_PID" >/dev/null 2>&1 || true
wait "$SERVER_PID" >/dev/null 2>&1 || true

echo "All audit checks passed."
