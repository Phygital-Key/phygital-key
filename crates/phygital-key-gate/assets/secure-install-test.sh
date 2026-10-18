#!/bin/bash
# shellcheck disable=SC2034 # every script carries the whole PRODUCT block; not all use every value
# CI check for the secure install (needs password-less sudo, as on CI runners).
# Generated for one product (the PRODUCT block) from the template in
# Phygital-Key/phygital-key, crates/phygital-key-gate/assets.
#   tests/secure-install.sh path/to/built/program
set -euo pipefail
BUILT="$1"
# BEGIN PRODUCT
P_NAME="Phygital Key"
P_BIN=phygital-key
P_ID=dev
P_REPO=Phygital-Key/phygital-key
P_PORT=$PORT
P_VERSION=v0.0.0
P_PATH=/agent
# END PRODUCT
PORT=$P_PORT
diag() {
  sudo tail -n 40 "/Library/Logs/PhygitalKey/$P_ID/gate.log" 2>/dev/null || true
  sudo journalctl -u "$P_BIN" -n 40 --no-pager 2>/dev/null || true
}
fail() { echo "FAIL: $*"; diag; exit 1; }
pass() { echo "PASS  $*"; }
case "$(uname -s)" in
  Darwin) SVC="_phygitalkey$P_ID"; DATA="/Library/Application Support/PhygitalKey/$P_ID"; PREFIX="/Library/PhygitalKey/$P_ID"
          UPD_PLIST="/Library/LaunchDaemons/io.phygitalkey.$P_ID.update.plist"; UPD_LABEL="io.phygitalkey.$P_ID.update" ;;
  Linux) SVC="phygitalkey-$P_ID"; DATA="/var/lib/$P_BIN"; PREFIX="/usr/lib/$P_BIN" ;;
esac
# The release for this version does not exist yet in CI, so hand over a root-owned
# copy, in a folder that is root-only on this machine.
case "$(uname -s)" in Darwin) CI_DIR=/Library/pka-ci ;; *) CI_DIR=/usr/lib/pka-ci ;; esac
sudo install -d -m 0755 -o root "$CI_DIR"
sudo install -m 0755 -o root "$BUILT" "$CI_DIR/$P_BIN"
INSTALL=(sudo "$CI_DIR/$P_BIN" install-service --binary "$CI_DIR/$P_BIN")

# A relative path is refused.
if (cd "$CI_DIR" && sudo "./$P_BIN" install-service --binary "$P_BIN") 2>/dev/null; then fail "accepted a relative --binary"; else pass "refuses a relative --binary"; fi
# A binary this user owns is refused.
if sudo "$BUILT" install-service --binary "$BUILT" 2>/dev/null; then fail "installed a user-owned binary"; else pass "refuses a binary the user could have changed"; fi

# A program grabbing the gate's address first makes the install refuse.
python3 -c '
import socket, sys, time
for fam, host in ((socket.AF_INET6, "::1"), (socket.AF_INET, "127.0.0.1")):
    try:
        s = socket.socket(fam); s.bind((host, int(sys.argv[1]))); s.listen(); break
    except OSError:
        continue
time.sleep(300)' "$PORT" &
HIJACK=$!
sleep 1
lsof -nP -iTCP:"$PORT" -sTCP:LISTEN -t 2>/dev/null | grep -qx "$HIJACK" || ss -ltnpH "sport = :$PORT" 2>/dev/null | grep -q "pid=$HIJACK" || fail "test setup: the fake program is not listening"
if "${INSTALL[@]}" >/dev/null 2>&1; then fail "installed while another program held the address"; else pass "refuses while another program holds the address"; fi
kill $HIJACK 2>/dev/null || true; wait $HIJACK 2>/dev/null || true
sleep 1

"${INSTALL[@]}" || fail "install did not succeed"
curl -fsS "http://localhost:$PORT/api/status" >/dev/null && pass "gate answers" || fail "gate answers"
for p in $(sudo lsof -nP -iTCP:"$PORT" -sTCP:LISTEN -t | sort -u); do
  [ "$(ps -o uid= -p "$p" | tr -d ' ')" = "$(id -u "$SVC")" ] || fail "a listener is not the gate user"
done
pass "only the gate user listens (IPv4 and IPv6)"
# Room for many connections: idle ones are dropped after 10 s, and the limit is
# high enough that a program cannot fill it with idle connections in that time.
GPID=$(sudo lsof -nP -iTCP:"$PORT" -sTCP:LISTEN -t | head -1)
if [ "$(uname -s)" = Linux ]; then nofile=$(sudo cat "/proc/$GPID/limits" | awk '/Max open files/ {print $4}')
else nofile=$(sudo plutil -extract SoftResourceLimits.NumberOfFiles raw "/Library/LaunchDaemons/io.phygitalkey.$P_ID.gate.plist" 2>/dev/null); fi
echo "      open file limit: ${nofile:-unknown}"
[ "${nofile:-0}" -ge 8192 ] 2>/dev/null && pass "the gate may open 8192 files" || fail "the gate's open file limit is ${nofile:-unknown}, want 8192"
owner() { if [ "$(uname -s)" = Darwin ]; then stat -f %Su "$1"; else stat -c %U "$1"; fi; }
[ "$(owner "$PREFIX/bin/$P_BIN")" = root ] && pass "program owned by root" || fail "program owned by root"
if touch "$PREFIX/bin/$P_BIN" 2>/dev/null; then fail "this user can change the program"; else pass "this user cannot change the program"; fi
if ls "$DATA" >/dev/null 2>&1; then fail "this user can read the data folder"; else pass "this user cannot read the data folder"; fi
if echo x > "$DATA/keyring.json" 2>/dev/null; then fail "this user can write the keyring"; else pass "this user cannot write the keyring"; fi
ADMIN="$PREFIX/bin/$P_BIN-admin"
status_out=$("$ADMIN" status) && grep -q "only the gate" <<< "$status_out" && pass "admin status confirms the listener" || fail "admin status confirms the listener"
link=$("$ADMIN" enroll | grep -o "http://localhost:$PORT/enroll#token=[A-Za-z0-9_-]*")
[ -n "$link" ] && pass "admin helper makes an enrollment link" || fail "admin helper makes an enrollment link"
logs_out=$("$ADMIN" logs 2>/dev/null || true)
if grep -q "token=" <<< "$logs_out"; then fail "a secret is in the gate log"; else pass "no enrollment link or token in the gate log"; fi
tok=$("$ADMIN" token | grep -o 'Bearer [A-Za-z0-9_-]*' | head -1 | cut -d' ' -f2)
code=$(curl -s -o /dev/null -w '%{http_code}' -X POST -H 'Content-Type: application/json' "http://localhost:$PORT$P_PATH" -d '{}')
[ "$code" = 401 ] && pass "agent endpoint refuses calls without the token" || fail "agent endpoint without token gave $code"
code=$(curl -s -o /dev/null -w '%{http_code}' -X POST -H "Authorization: bearer $tok" -H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream' "http://localhost:$PORT$P_PATH" -d '{}')
[ "$code" != 401 ] && [ "$code" != 403 ] && pass "agent endpoint accepts the token (any case of Bearer)" || fail "agent endpoint with token gave $code"

"${INSTALL[@]}" >/dev/null
curl -fsS "http://localhost:$PORT/api/status" >/dev/null && pass "installing again (update) works" || fail "installing again (update) works"

# Automatic updates: on by default, a root-owned job, refused when not signed.
BIN="$PREFIX/bin/$P_BIN"
updates_on() {
  if [ "$(uname -s)" = Darwin ]; then
    sudo test -f "$UPD_PLIST" &&
      [ "$(owner "$UPD_PLIST")" = root ] &&
      sudo launchctl print "system/$UPD_LABEL" >/dev/null 2>&1
  else
    systemctl is-enabled "$P_BIN-update.timer" >/dev/null 2>&1 &&
      [ "$(owner "/etc/systemd/system/$P_BIN-update.service")" = root ]
  fi
}
updates_on && pass "daily update job installed, owned by root" || fail "no daily update job"
[ "$(cat "$PREFIX/auto-update")" = on ] && pass "automatic updates on by default" || fail "automatic updates on by default"
T=$(mktemp -d)
printf 'phygital-key release manifest v1\nversion v9.9.9\n%064d  %s-v9.9.9-x86_64-unknown-linux-gnu.tar.gz\n' 0 "$P_BIN" > "$T/release.txt"
openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out "$T/k.pem" 2>/dev/null
openssl dgst -sha256 -sign "$T/k.pem" -out "$T/release.txt.sig" "$T/release.txt"
if "$BIN" release-check --file "$T/release.txt" --sig "$T/release.txt.sig" >/dev/null 2>&1; then fail "accepted a release list signed by an unknown key"; else pass "refuses a release list signed by an unknown key"; fi
rm -rf "$T"
before=$("$BIN" --version)
sudo "$BIN" install-service --auto-update-run >/dev/null 2>&1 || true
[ "$("$BIN" --version)" = "$before" ] && curl -fsS "http://localhost:$PORT/api/status" >/dev/null && pass "update job leaves the gate running unless a newer signed release exists" || fail "update job leaves the gate running unless a newer signed release exists"
sudo cat "$PREFIX/update-status" | grep -Eq "up to date|update check failed|update refused" && pass "update job records its result" || fail "no update result recorded"
# The daily job itself, started the way the timer starts it (not just the command
# with sudo, which brings its own environment), runs to the end.
sudo rm -f "$PREFIX/update-status"
if [ "$(uname -s)" = Linux ]; then
  sudo systemctl start "$P_BIN-update.service" || true
else
  sudo launchctl kickstart "system/$UPD_LABEL" || true
  for _ in $(seq 1 60); do sudo test -f "$PREFIX/update-status" && break; sleep 1; done
fi
if sudo cat "$PREFIX/update-status" 2>/dev/null | grep -Eq "up to date|update check failed|update refused" &&
  curl -fsS "http://localhost:$PORT/api/status" >/dev/null; then
  pass "the daily update job runs from its service manager"
else
  sudo journalctl -u "$P_BIN-update.service" -n 20 --no-pager 2>/dev/null || sudo tail -n 20 "/Library/Logs/PhygitalKey/$P_ID-update/update.log" 2>/dev/null || true
  fail "the daily update job did not run from its service manager"
fi
if [ "$(uname -s)" = Linux ]; then
  # systemd's own sandbox score: 0 is fully locked, 10 is open.
  for u in "$P_BIN.service" "$P_BIN-update.service"; do
    score=$(systemd-analyze security "$u" --no-pager 2>/dev/null | awk '/Overall exposure level/ {print $(NF-1)}')
    echo "      $u exposure $score"
    case "$u" in *-update.service) max=6.5 ;; *) max=2.5 ;; esac
    awk -v s="$score" -v m="$max" 'BEGIN { exit !(s != "" && s + 0 <= m) }' && pass "$u sandbox score $score (at most $max)" || fail "$u sandbox score $score, want at most $max"
  done
fi

# Root never takes the caller's environment: a planted ~/.curlrc (sudo keeps HOME on
# macOS) or a fake curl early in PATH must not run or write anything as root.
rm -f /tmp/pka-curlrc-planted /tmp/pka-path-planted
printf 'url = "https://github.com/"\noutput = "/tmp/pka-curlrc-planted"\n' > "$HOME/.curlrc"
FAKE=$(mktemp -d); printf '#!/bin/sh\ntouch /tmp/pka-path-planted\nexit 1\n' > "$FAKE/curl"; chmod +x "$FAKE/curl"
PATH="$FAKE:$PATH" sudo --preserve-env=HOME,PATH "$BIN" install-service --auto-update-run >/dev/null 2>&1 || true
PATH="$FAKE:$PATH" "$ADMIN" update >/dev/null 2>&1 || true
rm -f "$HOME/.curlrc"; rm -rf "$FAKE"
if [ -e /tmp/pka-curlrc-planted ]; then fail "root used the user's ~/.curlrc"; else pass "root ignores the user's ~/.curlrc"; fi
if [ -e /tmp/pka-path-planted ]; then fail "root ran a program from the user's PATH"; else pass "root ignores programs early in the user's PATH"; fi

# A new program that does not start is switched back to the previous one.
printf '#!/bin/sh\nexit 1\n' > /tmp/pka-broken
sudo install -m 0755 -o root /tmp/pka-broken "$CI_DIR/broken"; rm -f /tmp/pka-broken
if sudo "$CI_DIR/$P_BIN" install-service --binary "$CI_DIR/broken" >/dev/null 2>&1; then fail "installed a program that does not start"; fi
[ "$("$BIN" --version)" = "$before" ] && curl -fsS "http://localhost:$PORT/api/status" >/dev/null && pass "a failed update switches back to the previous program" || fail "no switch back after a failed update"

# Turning updates off is remembered on the next install.
"${INSTALL[@]}" --no-auto-update >/dev/null
if updates_on; then fail "update job still there after --no-auto-update"; else pass "--no-auto-update removes the job"; fi
"${INSTALL[@]}" >/dev/null
if updates_on; then fail "a later install turned updates back on"; else pass "the off choice is kept on later installs"; fi
"${INSTALL[@]}" --auto-update >/dev/null
updates_on && pass "--auto-update turns it back on" || fail "--auto-update did not turn it on"
sudo "$PREFIX/bin/$P_BIN" install-service --uninstall
sleep 1
if curl -fsS "http://localhost:$PORT/api/status" >/dev/null 2>&1; then fail "still running after uninstall"; else pass "uninstall stops it"; fi
sudo test -d "$DATA" && pass "uninstall keeps keys" || fail "uninstall keeps keys"
sudo "$CI_DIR/$P_BIN" install-service --uninstall --purge
if sudo test -d "$DATA"; then fail "purge left data"; else pass "purge removes everything"; fi
if updates_on; then fail "update job left after uninstall"; else pass "uninstall removes the update job"; fi
echo "secure install: all checks passed"
