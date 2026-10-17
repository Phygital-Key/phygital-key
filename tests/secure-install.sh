#!/bin/bash
# CI check for `sudo phygital-key install-service` (needs passwordless sudo, as on CI runners).
set -euo pipefail
BIN="$1"
fail() { echo "FAIL: $*"; exit 1; }
pass() { echo "PASS  $*"; }

sudo "$BIN" install-service
case "$(uname -s)" in
  Darwin) SVC=_phygitalkey; DATA="/Library/Application Support/PhygitalKey" ;;
  Linux) SVC=phygitalkey; DATA=/var/lib/phygital-key ;;
esac

curl -fsS http://localhost:7468/api/status >/dev/null && pass "gate answers"
owner=$(ps -o user= -p "$(pgrep -f 'libexec/phygital-key/phygital-key' | head -1)" | tr -d ' ')
[ "$owner" = "$SVC" ] && pass "gate runs as $SVC" || fail "gate runs as '$owner'"
[ "$(stat -f %Su /usr/local/libexec/phygital-key/phygital-key 2>/dev/null || stat -c %U /usr/local/libexec/phygital-key/phygital-key)" = root ] && pass "program owned by root"
if touch /usr/local/libexec/phygital-key/phygital-key 2>/dev/null; then fail "this user can change the program"; else pass "this user cannot change the program"; fi
if ls "$DATA" >/dev/null 2>&1; then fail "this user can read the data folder"; else pass "this user cannot read the data folder"; fi
if echo x > "$DATA/keyring.json" 2>/dev/null; then fail "this user can write the keyring"; else pass "this user cannot write the keyring"; fi
link=$(phygital-key-admin enroll | grep -o 'http://localhost:7468/enroll#token=[A-Za-z0-9_-]*')
[ -n "$link" ] && pass "admin helper makes an enrollment link"
token_line=$(phygital-key-admin token)
echo "$token_line" | grep -q "Bearer " && pass "admin helper shows the MCP token"
tok=$(echo "$token_line" | grep -o 'Bearer [A-Za-z0-9_-]*' | head -1 | cut -d' ' -f2)
code=$(curl -s -o /dev/null -w '%{http_code}' -X POST -H 'Content-Type: application/json' http://localhost:7468/mcp -d '{}')
[ "$code" = 401 ] && pass "MCP refuses calls without the token" || fail "MCP without token gave $code"
code=$(curl -s -o /dev/null -w '%{http_code}' -X POST -H "Authorization: Bearer $tok" -H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream' http://localhost:7468/mcp \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"ci","version":"1"}}}')
[ "$code" = 200 ] && pass "MCP accepts the token" || fail "MCP with token gave $code"

sudo "$BIN" install-service
curl -fsS http://localhost:7468/api/status >/dev/null && pass "installing again (update) works"

sudo "$BIN" install-service --uninstall
sleep 1
if curl -fsS http://localhost:7468/api/status >/dev/null 2>&1; then fail "still running after uninstall"; else pass "uninstall stops it"; fi
sudo test -d "$DATA" && pass "uninstall keeps keys"
sudo "$BIN" install-service --uninstall --purge
sudo test -d "$DATA" && fail "purge left data" || pass "purge removes everything"
echo "secure install: all checks passed"
