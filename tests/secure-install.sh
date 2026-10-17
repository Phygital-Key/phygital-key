#!/bin/bash
# CI check for the secure install (needs password-less sudo, as on CI runners).
set -euo pipefail
BUILT="$1"
fail() { echo "FAIL: $*"; exit 1; }
pass() { echo "PASS  $*"; }
case "$(uname -s)" in
  Darwin) SVC=_phygitalkey; DATA="/Library/Application Support/PhygitalKey"; PREFIX=/Library/PhygitalKey ;;
  Linux) SVC=phygitalkey; DATA=/var/lib/phygital-key; PREFIX=/opt/phygital-key ;;
esac
# The release for this version does not exist yet in CI, so hand over a root-owned copy.
sudo install -d -m 0755 -o root /opt/pka-ci
sudo install -m 0755 -o root "$BUILT" /opt/pka-ci/phygital-key
INSTALL=(sudo /opt/pka-ci/phygital-key install-service --binary /opt/pka-ci/phygital-key)

# A binary this user owns is refused.
if sudo "$BUILT" install-service --binary "$BUILT" 2>/dev/null; then fail "installed a user-owned binary"; else pass "refuses a binary the user could have changed"; fi

# A program grabbing the gate's address first makes the install refuse.
python3 -c 'import socket,time; s=socket.socket(socket.AF_INET6); s.bind(("::1",7468)); s.listen(); time.sleep(30)' &
HIJACK=$!
sleep 1
if "${INSTALL[@]}" >/dev/null 2>&1; then fail "installed while another program held the address"; else pass "refuses while another program holds the address"; fi
kill $HIJACK 2>/dev/null || true; wait $HIJACK 2>/dev/null || true
sleep 1

"${INSTALL[@]}"
curl -fsS http://localhost:7468/api/status >/dev/null && pass "gate answers"
for p in $(sudo lsof -nP -iTCP:7468 -sTCP:LISTEN -t | sort -u); do
  [ "$(ps -o user= -p "$p" | tr -d ' ')" = "$SVC" ] || fail "a listener is not the gate user"
done
pass "only the gate user listens (IPv4 and IPv6)"
[ "$(stat -f %Su "$PREFIX/bin/phygital-key" 2>/dev/null || stat -c %U "$PREFIX/bin/phygital-key")" = root ] && pass "program owned by root"
if touch "$PREFIX/bin/phygital-key" 2>/dev/null; then fail "this user can change the program"; else pass "this user cannot change the program"; fi
if ls "$DATA" >/dev/null 2>&1; then fail "this user can read the data folder"; else pass "this user cannot read the data folder"; fi
if echo x > "$DATA/keyring.json" 2>/dev/null; then fail "this user can write the keyring"; else pass "this user cannot write the keyring"; fi
ADMIN="$PREFIX/bin/phygital-key-admin"
"$ADMIN" status | grep -q "only the gate" && pass "admin status confirms the listener"
link=$("$ADMIN" enroll | grep -o 'http://localhost:7468/enroll#token=[A-Za-z0-9_-]*')
[ -n "$link" ] && pass "admin helper makes an enrollment link"
if sudo grep -q "token=" /Library/Logs/PhygitalKey/gate.log /var/log/phygital-key/gate.log 2>/dev/null; then fail "a secret is in the gate log"; else pass "no enrollment link or token in the gate log"; fi
tok=$("$ADMIN" token | grep -o 'Bearer [A-Za-z0-9_-]*' | head -1 | cut -d' ' -f2)
code=$(curl -s -o /dev/null -w '%{http_code}' -X POST -H 'Content-Type: application/json' http://localhost:7468/mcp -d '{}')
[ "$code" = 401 ] && pass "MCP refuses calls without the token" || fail "MCP without token gave $code"
code=$(curl -s -o /dev/null -w '%{http_code}' -X POST -H "Authorization: bearer $tok" -H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream' http://localhost:7468/mcp \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"ci","version":"1"}}}')
[ "$code" = 200 ] && pass "MCP accepts the token (any case of Bearer)" || fail "MCP with token gave $code"

"${INSTALL[@]}" >/dev/null
curl -fsS http://localhost:7468/api/status >/dev/null && pass "installing again (update) works"
sudo "$PREFIX/bin/phygital-key" install-service --uninstall
sleep 1
if curl -fsS http://localhost:7468/api/status >/dev/null 2>&1; then fail "still running after uninstall"; else pass "uninstall stops it"; fi
sudo test -d "$DATA" && pass "uninstall keeps keys"
sudo /opt/pka-ci/phygital-key install-service --uninstall --purge
if sudo test -d "$DATA"; then fail "purge left data"; else pass "purge removes everything"; fi
echo "secure install: all checks passed"
