#!/bin/bash
# shellcheck disable=SC2034 # BIN, P_BIN and RELEASE_KEYS are read by verify_release
# The secure installer's release-list check (verify_release in
# assets/install-service.sh), run on its own against lists signed with a
# throwaway test key: v2 lists in date, expired, from the future, changed or
# badly formed, and an old v1 list. Needs bash and openssl. No root, no network.
#   crates/phygital-key-gate/tests/release-lists.sh
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
src="$here/../assets/install-service.sh"
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
fails=0
pass() { echo "PASS  $*"; }
fail() { echo "FAIL  $*"; fails=$((fails + 1)); }

# Only the function, with stand-ins for what the installer has around it.
sed -n '/^verify_release() {$/,/^}$/p; /^seen_signed() /p; /^remember_signed() {$/,/^}$/p' "$src" > "$T/fn.sh"
[ -s "$T/fn.sh" ] || { echo "verify_release not found in $src"; exit 1; }
P_BIN=phygital-key-test
BIN=/nonexistent/program
die() { echo "die: $*" >&2; exit 1; }
root_only() { return 1; }
openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out "$T/key.pem" 2>/dev/null
RELEASE_KEYS="$(openssl pkey -in "$T/key.pem" -pubout 2>/dev/null)"
# shellcheck source=/dev/null
. "$T/fn.sh"

now=$(date +%s)
file="0000000000000000000000000000000000000000000000000000000000000000  $P_BIN-v0.5.0-x86_64-unknown-linux-gnu.tar.gz"
v2() { printf 'phygital-key release manifest v2\nversion v0.5.0\nsigned %s\nexpires %s\n%s\n' "$1" "$2" "$file"; }
# case NAME LIST-TEXT [v1]: writes the list and signs it with the test key.
mk() {
  local d="$T/$1"; mkdir -p "$d"
  local name=release2.txt; [ "${3:-}" = v1 ] && name=release.txt
  printf '%s\n' "$2" > "$d/$name"
  openssl dgst -sha256 -sign "$T/key.pem" -out "$d/$name.sig" "$d/$name"
  echo "$d"
}
accepts() { local out; if out=$(verify_release "$2" 2>/dev/null); then [ "$out" = "$3" ] && pass "$1" || fail "$1 (printed '$out', want '$3')"; else fail "$1 (refused)"; fi; }
refuses() { if (verify_release "$2" >/dev/null 2>&1); then fail "$1 (accepted)"; else pass "$1"; fi; }

s=$((now - 60)); e=$((now + 180 * 86400))
accepts "a v2 list in date is accepted, with its signed and expires times" "$(mk good "$(v2 $s $e)")" "v0.5.0 $s $e"
refuses "an expired v2 list is refused" "$(mk expired "$(v2 $((now - 200)) $((now - 100)))")"
refuses "a v2 list signed more than a day in the future is refused" "$(mk future "$(v2 $((now + 2 * 86400)) $((now + 3 * 86400)))")"
refuses "a v2 list good for more than 400 days is refused" "$(mk long "$(v2 $s $((s + 401 * 86400)))")"
refuses "expires before signed is refused" "$(mk backwards "$(v2 $s $((s - 1)))")"
refuses "a leading zero in a time is refused" "$(mk zero "$(v2 "0$s" $e)")"
d=$(mk changed "$(v2 $s $e)"); v2 $s $((e + 1)) > "$d/release2.txt"
refuses "a v2 list changed after signing is refused" "$d"
d=$(mk otherkey "$(v2 $s $e)")
openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out "$T/other.pem" 2>/dev/null
openssl dgst -sha256 -sign "$T/other.pem" -out "$d/release2.txt.sig" "$d/release2.txt"
refuses "a v2 list signed by another key is refused" "$d"
refuses "a v2 list with a v1 header is refused" "$(mk header "$(v2 $s $e | sed 's/manifest v2/manifest v1/')")"
refuses "a v2 list with no files is refused" "$(mk nofiles "$(v2 $s $e | sed '$d')")"
refuses "a v2 list with a file for another version is refused" "$(mk otherversion "$(v2 $s $e | sed 's/-v0.5.0-/-v0.4.0-/')")"
accepts "an old v1 list (an older release asked for by version) is still read" \
  "$(mk v1 "$(printf 'phygital-key release manifest v1\nversion v0.5.0\n%s\n' "$file")" v1)" "v0.5.0"

# With an installed program (a stand-in that records how it was called): it is
# asked to check release2.txt too, with the ML-DSA signature when there is one.
cat > "$T/prog" <<'PROG'
#!/bin/bash
[ "$1" = release-check ] || exit 1
printf '%s\n' "$*" >> "${0%/*}/calls"
PROG
chmod +x "$T/prog"
BIN="$T/prog"; root_only() { return 0; }
d=$(mk withprog "$(v2 $s $e)")
accepts "with an installed program and no ML-DSA file, the check still runs (bash 3.2 too)" "$d" "v0.5.0 $s $e"
grep -q -- "--file $d/release2.txt --sig $d/release2.txt.sig$" "$T/calls" && pass "the program was asked to check release2.txt" || fail "the program was not asked: $(cat "$T/calls" 2>/dev/null)"
: > "$d/release2.txt.mldsa"
accepts "with an ML-DSA file, the check runs" "$d" "v0.5.0 $s $e"
grep -q -- "--pq-sig $d/release2.txt.mldsa" "$T/calls" && pass "the ML-DSA signature is handed to the program" || fail "no --pq-sig: $(cat "$T/calls")"
printf '#!/bin/bash\necho "not signed by the pinned ML-DSA release key"; [ "$2" = --help ] && echo --pq-sig; exit 1\n' > "$T/prog"
refuses "when the program refuses, the list is refused" "$d"
printf '#!/bin/bash\n[ "$2" = --help ] && { echo "--file --sig"; exit 0; }; echo "unknown format"; exit 1\n' > "$T/prog"
accepts "an older program (no --pq-sig) cannot read release2.txt: the P-256 check stands" "$d" "v0.5.0 $s $e"
BIN=/nonexistent/program; root_only() { return 1; }

# Rollback memory: the newest signed time is kept, never an older one.
PREFIX="$T/prefix"; mkdir -p "$PREFIX"
[ -z "$(seen_signed)" ] && pass "nothing remembered at first" || fail "nothing remembered at first"
remember_signed 2000; remember_signed 1000
[ "$(seen_signed)" = 2000 ] && pass "an older signed time does not replace a newer one" || fail "remembered $(seen_signed), want 2000"
remember_signed 3000
[ "$(seen_signed)" = 3000 ] && pass "a newer signed time is remembered" || fail "remembered $(seen_signed), want 3000"
echo junk > "$PREFIX/release-seen"
[ -z "$(seen_signed)" ] && pass "a damaged memory file reads as nothing" || fail "read '$(seen_signed)' from junk"

echo
[ "$fails" = 0 ] && echo "release lists: all checks passed" || { echo "release lists: $fails failed"; exit 1; }
