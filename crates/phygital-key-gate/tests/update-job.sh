#!/bin/bash
# shellcheck disable=SC2034 # the variables below are read by the functions under test
# Two parts of the secure installer's daily update job (assets/install-service.sh),
# run on their own with stand-ins for curl, kill and the service manager:
#   standing_hold  waits while a standing approval is active, 7 days at most
#   gate_up        stops a program that took the gate's port during the restart
# No root, no network.
#   crates/phygital-key-gate/tests/update-job.sh
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
src="$here/../assets/install-service.sh"
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
fails=0
pass() { echo "PASS  $*"; }
fail() { echo "FAIL  $*"; fails=$((fails + 1)); }

# ours() reads /proc/<pid>/cgroup; here it reads a stand-in folder instead.
sed -n '/^standing_hold() {$/,/^}$/p; /^gate_up() {$/,/^}$/p; /^ours() {$/,/^}$/p' "$src" | sed "s|/proc/|$T/proc/|g" > "$T/fn.sh"
[ "$(grep -c '^}$' "$T/fn.sh")" = 3 ] || { echo "standing_hold, gate_up or ours not found in $src"; exit 1; }
# shellcheck source=/dev/null
. "$T/fn.sh"

P_BIN=phygital-key-test; PORT=17999; HAVE=v0.5.0; FORCE=no
PREFIX="$T/prefix"; mkdir -p "$PREFIX"
note() { printf '%s\n' "$*" > "$T/note"; }
sleep() { :; }

# --- standing_hold. The gate's status says how many standing approvals are active.
ACTIVE=0
curl() { printf '{"keys":[],"standing_active": %s}' "$ACTIVE"; }
if standing_hold v0.6.0; then fail "holds with no standing approval"; else pass "goes on when no standing approval is active"; fi
ACTIVE=2
if standing_hold v0.6.0; then pass "waits while standing approvals are active"; else fail "did not wait with 2 active"; fi
grep -q "waiting to install v0.6.0: 2 standing approval(s) active" "$T/note" && pass "says why it waits, and how to install now" || fail "note was: $(cat "$T/note")"
[ "$(awk '{print $2}' "$PREFIX/update-postponed")" = "$HAVE" ] && pass "remembers when it first waited, for the installed version" || fail "no postponed record"
printf '%s %s\n' $(( $(date +%s) - 6 * 86400 )) "$HAVE" > "$PREFIX/update-postponed"
if standing_hold v0.6.0; then pass "still waits after 6 days"; else fail "stopped waiting after 6 days"; fi
printf '%s %s\n' $(( $(date +%s) - 7 * 86400 - 1 )) "$HAVE" > "$PREFIX/update-postponed"
if standing_hold v0.6.0; then fail "waited longer than 7 days"; else pass "goes on after 7 days, so a security fix still lands"; fi
printf '%s %s\n' $(( $(date +%s) - 8 * 86400 )) v0.4.0 > "$PREFIX/update-postponed"
if standing_hold v0.6.0; then pass "an old record for another version does not count"; else fail "used the record of another version"; fi
FORCE=yes
if standing_hold v0.6.0; then fail "waited with --now"; else pass "--now does not wait"; fi
FORCE=no
curl() { return 7; }
if standing_hold v0.6.0; then fail "waited when the gate did not answer"; else pass "goes on when the gate does not answer (nothing to lose)"; fi

# --- gate_up. A stand-in port: "foreign" while a squatter holds it, then the gate.
squatter_left=0      # how many more times a squatter grabs the port
gate_running=no
listener_pids() { [ "$gate_running" = yes ] || [ "$squatter_left" -gt 0 ] && echo 4242; }
foreign_listener() { [ "$squatter_left" -gt 0 ] && echo "uid 501, pid 4242"; }
foreign_pids() { [ "$squatter_left" -gt 0 ] && echo 4242; }
kill() { squatter_left=$((squatter_left - 1)); echo "$*" >> "$T/kills"; }
start_gate() { gate_running=yes; }
curl() { [ "$gate_running" = yes ] && [ "$squatter_left" = 0 ]; }
refuse() { echo "$*" > "$T/refused"; return 99; }

run_gate_up() { : > "$T/kills"; rm -f "$T/refused"; KILLED=""; gate_running=yes; gate_up; }
AUTO_RUN=yes; squatter_left=2
if run_gate_up; then pass "the daily job stops a program that took the port, and the gate comes up"; else fail "gate_up failed with 2 squatters"; fi
[ "$(wc -l < "$T/kills" | tr -d ' ')" = 2 ] && grep -q -- "-9 4242" "$T/kills" && pass "it was stopped with kill -9, once per grab" || fail "kills: $(cat "$T/kills")"
[ "$KILLED" = " [uid 501, pid 4242] [uid 501, pid 4242]" ] && pass "the result line names every program it stopped" || fail "KILLED was '$KILLED'"
AUTO_RUN=yes; squatter_left=9
if (run_gate_up) 2>/dev/null; then fail "kept fighting a squatter for good"; else pass "after 5 tries the gate is stopped (fail closed)"; fi
AUTO_RUN=no; squatter_left=1
if (run_gate_up) 2>/dev/null; then fail "an install by hand fought the squatter"; else pass "an install by hand refuses instead of stopping programs"; fi

# --- ours(): which listeners on the port belong to the gate's own system units.
OS=linux; P_BIN=phygital-key-test; SVC_UID=995
cg() { mkdir -p "$T/proc/$1"; printf '%s\n' "$2" > "$T/proc/$1/cgroup"; }
ps() { case "${@: -1}" in 20) echo 995 ;; 21|22) echo 0 ;; *) echo 1000 ;; esac; }
cg 20 "0::/system.slice/phygital-key-test.service"
cg 21 "0::/system.slice/phygital-key-test.socket"
cg 22 "0::/system.slice/other.service"
cg 30 "0::/user.slice/user-1000.slice/user@1000.service/app.slice/phygital-key-test.service"
cg 31 "0::/system.slice/phygital-key-test.service"
ours 1 && pass "systemd (pid 1) holds the port for the gate" || fail "pid 1 not ours"
ours 20 && pass "the gate in its system unit is ours" || fail "the gate is not ours"
ours 21 && pass "systemd's helper (root) in the gate's units is ours" || fail "the helper is not ours"
if ours 22; then fail "root in another unit counted as ours"; else pass "root in another unit is not ours"; fi
if ours 30; then fail "a user unit with the same name counted as ours"; else pass "a user's own unit named like the gate is not ours"; fi
if ours 31; then fail "another user's process in the gate's cgroup path counted as ours"; else pass "a process of another user is not ours, whatever its cgroup says"; fi
OS=mac
if ours 20; then fail "ours() said yes on macOS"; else pass "on macOS nothing is skipped"; fi

echo
[ "$fails" = 0 ] && echo "update job: all checks passed" || { echo "update job: $fails failed"; exit 1; }
