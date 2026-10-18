#!/bin/bash
# shellcheck disable=SC2034 # every script carries the whole PRODUCT block; not all use every value
# Phygital Key: run the gate as its own locked-down system user (macOS, Linux).
# This file is generated for one product (the PRODUCT block below) from the
# template in Phygital-Key/phygital-key, crates/phygital-key-gate/assets.
#
# Recommended (from a fresh terminal you trust), with the product's own copy:
#   curl -q -fsSL https://raw.githubusercontent.com/<product repo>/<tag>/install-service.sh | sudo bash
# It then updates itself once a day to the latest SIGNED release (see below).
# Later, use the locked copy it installs:
#   sudo <prefix>/bin/<program> install-service [--release vX.Y.Z | --no-auto-update | --uninstall]
# Shortcut: `sudo <program> install-service` runs YOUR copy of the program as
# administrator. Only use it if nothing else could have changed that file.
#
# Options:  --uninstall        remove the service (keeps keys and log)
#           --purge            with --uninstall: also delete the gate user, keys and log
#           --no-auto-update   no daily update (kept on later installs until --auto-update)
#           --auto-update      daily update to the latest signed release (the default)
#           (with curl ... | sudo bash -s -- --no-auto-update)
#
# Why: an AI agent runs as you. If the gate also ran as you, the agent could edit
# the gate's program, keys and log. Here the gate runs as a separate user. Its
# program is the official release, whose file list and SHA-256 sums are signed with
# the product's release key (used only in release runs for owner-only v* tags), and it
# is kept in folders only an administrator can change. Its data folder is closed to everyone else. Nothing that you (or an agent acting as you) could have edited is
# ever run as root or installed.
set -euo pipefail
# Root must not take anything from the caller's environment: a ~/.curlrc in HOME
# (sudo keeps HOME on macOS) could make curl write any file, and a folder the user
# can write in PATH could hold a fake curl, awk or openssl.
export PATH=/usr/bin:/bin:/usr/sbin:/sbin
HOME=$(cd ~root && pwd); export HOME
unset CURL_HOME XDG_CONFIG_HOME SSL_CERT_FILE SSL_CERT_DIR CURL_CA_BUNDLE OPENSSL_CONF OPENSSL_MODULES BASH_ENV ENV LD_PRELOAD LD_LIBRARY_PATH DYLD_INSERT_LIBRARIES DYLD_LIBRARY_PATH
umask 022

# The product this copy installs (filled in when the product is built).
# BEGIN PRODUCT
P_NAME="Phygital Key"
P_BIN=phygital-key
P_ID=dev
P_REPO=Phygital-Key/phygital-key
P_PORT=7468
P_VERSION=v0.0.0
# END PRODUCT
[[ "$P_ID" =~ ^[a-z0-9]+$ ]] && [[ "$P_BIN" =~ ^[a-z0-9-]+$ ]] && [[ "$P_REPO" =~ ^[A-Za-z0-9-]+/[A-Za-z0-9._-]+$ ]] &&
  [[ "$P_NAME" =~ ^[A-Za-z0-9\ ]+$ ]] && [[ "$P_PORT" =~ ^[0-9]{1,5}$ ]] && [[ "$P_VERSION" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] ||
  { echo "bad product block" >&2; exit 1; }
REPO="$P_REPO"
VERSION="${PKA_VERSION:-$P_VERSION}"
PORT="${PKA_PORT:-$P_PORT}"
BINARY="${PKA_BINARY:-}"   # optional: an already root-owned binary (used by CI)
MODE=install
PURGE=no
AUTO_PREF=""      # on / off when asked for, else keep the last choice (default on)
AUTO_RUN=no       # the daily update job
FORCE=no          # with --auto-update-run: do not wait for standing approvals
for a in "$@"; do
  case "$a" in
    --uninstall) MODE=uninstall ;;
    --purge) PURGE=yes ;;
    --auto-update) AUTO_PREF=on ;;
    --no-auto-update) AUTO_PREF=off ;;
    --auto-update-run) AUTO_RUN=yes ;;
    --now) FORCE=yes ;;
    *) echo "unknown option: $a" >&2; exit 2 ;;
  esac
done
# Pinned release keys (P-256). A release installs only if its release.txt is signed
# by one of these. scripts/release-key.sh writes this block; the program reads the
# same block, so the shell installer and `<program> update` trust the same keys.
# BEGIN RELEASE KEYS
RELEASE_KEYS='
'
# END RELEASE KEYS

say() { printf '%s\n' "$*"; }
die() {
  # The daily job leaves its result for <program>-admin status.
  [ "${AUTO_RUN:-no}" = yes ] && declare -F note >/dev/null && note "update stopped: $*" >/dev/null
  printf '%s install-service: %s\n' "$P_BIN" "$*" >&2; exit 1
}
[ "$(id -u)" = 0 ] || die "needs administrator rights (sudo)."
[[ "$VERSION" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "bad version: $VERSION"
[[ "$PORT" =~ ^[0-9]{1,5}$ ]] && [ "$PORT" -ge 1 ] && [ "$PORT" -le 65535 ] || die "bad port: $PORT"
case "$BINARY" in ""|/*) ;; *) die "--binary needs a full path (starting with /)" ;; esac

case "$(uname -s)" in
  Darwin)
    OS=mac; SVC_USER="_phygitalkey$P_ID"
    PREFIX="/Library/PhygitalKey/$P_ID"
    DATA="/Library/Application Support/PhygitalKey/$P_ID"
    LOGS="/Library/Logs/PhygitalKey/$P_ID"
    LABEL="io.phygitalkey.$P_ID"
    UNIT="/Library/LaunchDaemons/$LABEL.gate.plist"
    UPD_UNIT="/Library/LaunchDaemons/$LABEL.update.plist"
    UPD_LOGS="/Library/Logs/PhygitalKey/$P_ID-update"
    TRIPLE_OS=apple-darwin ;;
  Linux)
    OS=linux; SVC_USER="phygitalkey-$P_ID"
    PREFIX="/usr/lib/$P_BIN"
    DATA="/var/lib/$P_BIN"
    LOGS=""
    UNIT="/etc/systemd/system/$P_BIN.service"
    UPD_UNIT="/etc/systemd/system/$P_BIN-update.service"
    UPD_TIMER="/etc/systemd/system/$P_BIN-update.timer"
    UPD_LOGS=""
    TRIPLE_OS=unknown-linux-gnu ;;
  *) die "only macOS and Linux for now." ;;
esac
BIN="$PREFIX/bin/$P_BIN"
ADMIN="$PREFIX/bin/$P_BIN-admin"
LINK="/usr/local/bin/$P_BIN-admin"

owner_of() { if [ "$OS" = mac ]; then stat -f %Su "$1"; else stat -c %U "$1"; fi; }
mode_of() { if [ "$OS" = mac ]; then stat -f %Lp "$1"; else stat -c %a "$1"; fi; }
# True if $1 and every folder above it belong to root and no one else can write them.
root_only() {
  local p="$1"
  while :; do
    [ -e "$p" ] || { echo "  not found: $p" >&2; return 1; }
    [ ! -L "$p" ] || { echo "  is a link: $p" >&2; return 1; }
    local o m; o=$(owner_of "$p"); m=$(mode_of "$p")
    [ "$o" = root ] || { echo "  owned by $o, not root: $p" >&2; return 1; }
    # No group or other write bit (the last three digits of the mode).
    m=${m: -3}
    [ $(( 8#$m & 8#022 )) -eq 0 ] || { echo "  writable by others (mode $m): $p" >&2; return 1; }
    [ "$p" = / ] && return 0
    p=$(dirname "$p")
  done
}

# Process ids listening on a TCP port (IPv4 and IPv6).
listener_pids() {
  if command -v lsof >/dev/null 2>&1; then
    lsof -nP -iTCP:"$1" -sTCP:LISTEN -t 2>/dev/null | sort -u
  else
    ss -ltnpH "sport = :$1" 2>/dev/null | grep -o 'pid=[0-9]*' | cut -d= -f2 | sort -u
  fi
}

# Is $1 newer than $2? (both vX.Y.Z, checked before)
newer() {
  local a1 a2 a3 b1 b2 b3
  IFS=. read -r a1 a2 a3 <<< "${1#v}"
  IFS=. read -r b1 b2 b3 <<< "${2#v}"
  (( 10#$a1 > 10#$b1 || (10#$a1 == 10#$b1 && (10#$a2 > 10#$b2 || (10#$a2 == 10#$b2 && 10#$a3 > 10#$b3))) ))
}

# Check $1/release.txt against $1/release.txt.sig with the pinned keys.
# Prints the signed version. Fails on any doubt.
verify_release() {
  local d="$1" n=0 ok=no v
  if command -v openssl >/dev/null 2>&1; then
    printf '%s\n' "$RELEASE_KEYS" | awk -v dir="$d" '
      /-----BEGIN PUBLIC KEY-----/ { n++; f = dir "/key-" n ".pem" }
      f { print > f }
      /-----END PUBLIC KEY-----/ { close(f); f = "" }'
    for k in "$d"/key-*.pem; do
      [ -f "$k" ] || continue
      n=$((n + 1))
      if openssl dgst -sha256 -verify "$k" -signature "$d/release.txt.sig" "$d/release.txt" >/dev/null 2>&1; then ok=yes; fi
    done
    [ "$n" -gt 0 ] || { echo "  no release key is pinned in this installer" >&2; return 1; }
    [ "$ok" = yes ] || { echo "  release.txt is not signed by a pinned release key" >&2; return 1; }
    [ "$(sed -n 1p "$d/release.txt")" = "phygital-key release manifest v1" ] || return 1
    v=$(sed -n 2p "$d/release.txt" | sed -n 's/^version \(v[0-9][0-9]*\.[0-9][0-9]*\.[0-9][0-9]*\)$/\1/p')
    [ -n "$v" ] || return 1
    # The same strict format the program checks: LF only, a final LF, at least one
    # file, every file line exact, no name twice.
    [ -z "$(tail -c 1 "$d/release.txt")" ] || return 1
    ! LC_ALL=C grep -q "$(printf '\r')" "$d/release.txt" || return 1
    [ "$(sed -n '3,$p' "$d/release.txt" | wc -l | tr -d ' ')" -ge 1 ] || return 1
    [ "$(sed -n '3,$p' "$d/release.txt" | LC_ALL=C grep -cvE "^[0-9a-f]{64}  ${P_BIN}-${v//./\\.}-[A-Za-z0-9._-]+\.(tar\.gz|zip)\$" || true)" = 0 ] || return 1
    [ -z "$(sed -n '3,$p' "$d/release.txt" | awk '{ print $2 }' | sort | uniq -d)" ] || return 1
  elif [ -x "$BIN" ] && root_only "$BIN" 2>/dev/null; then
    # The installed, root-owned program checks it with the same pinned keys.
    v=$("$BIN" release-check --file "$d/release.txt" --sig "$d/release.txt.sig") || return 1
  else
    die "needs openssl to check the release signature. Install openssl, then run this again."
  fi
  [[ "$v" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] || return 1
  printf '%s\n' "$v"
}

fetch_release_list() { # $1 = base URL, $2 = folder
  curl -q --proto '=https' --tlsv1.2 --max-time 120 -fsSL "$1/release.txt" -o "$2/release.txt" &&
  curl -q --proto '=https' --tlsv1.2 --max-time 120 -fsSL "$1/release.txt.sig" -o "$2/release.txt.sig"
}

# The daily update job's last result, for <program>-admin status (root-only folder).
note() {
  [ -n "${PREFIX:-}" ] && [ -d "$PREFIX" ] || return 0
  printf '%s %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*" > "$PREFIX/update-status.new" &&
    chmod 0644 "$PREFIX/update-status.new" && mv -f "$PREFIX/update-status.new" "$PREFIX/update-status"
  say "$*"
}

stop_updates() {
  if [ "$OS" = mac ]; then
    launchctl bootout "system/$LABEL.update" 2>/dev/null || true
    rm -f "$UPD_UNIT"
  else
    systemctl disable --now "$P_BIN-update.timer" 2>/dev/null || true
    rm -f "$UPD_TIMER" "$UPD_UNIT"
    systemctl daemon-reload 2>/dev/null || true
  fi
}

stop_service() {
  if [ "$OS" = mac ]; then launchctl bootout "system/$LABEL.gate" 2>/dev/null || true
  else systemctl disable --now "$P_BIN.service" 2>/dev/null || true; fi
}

if [ "$MODE" = uninstall ]; then
  stop_updates
  stop_service
  rm -f "$UNIT"
  [ -L "$LINK" ] && [ "$(readlink "$LINK")" = "$ADMIN" ] && rm -f "$LINK"
  rm -rf "$PREFIX"
  [ "$OS" = linux ] && systemctl daemon-reload || true
  if [ "$PURGE" = yes ]; then
    rm -rf "$DATA"; [ -n "$LOGS" ] && rm -rf "$LOGS"; [ -n "$UPD_LOGS" ] && rm -rf "$UPD_LOGS"
    if [ "$OS" = mac ]; then
      dscl . -delete "/Users/$SVC_USER" 2>/dev/null || true
      dscl . -delete "/Groups/$SVC_USER" 2>/dev/null || true
    else userdel "$SVC_USER" 2>/dev/null || true; fi
    say "Removed $P_NAME, its user, keys and log."
  else
    say "Removed the $P_NAME service. Your keys and log are kept in: $DATA"
  fi
  exit 0
fi

WORK=$(mktemp -d "/tmp/$P_BIN-install.XXXXXX")
chmod 700 "$WORK"
trap 'rm -rf "$WORK"' EXIT
REL="$WORK/release"
mkdir -m 700 "$REL"
HAVE=""
[ -x "$BIN" ] && HAVE=$("$BIN" --version 2>/dev/null | awk '{print "v" $2}') || true

# 0. The daily update job: go on only if a newer signed release exists.
if [ "$AUTO_RUN" = yes ]; then
  [ -z "$BINARY" ] || die "--auto-update-run downloads the release itself."
  [[ "$HAVE" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "the secure install is not here: $BIN"
  if ! fetch_release_list "https://github.com/$REPO/releases/latest/download" "$REL"; then
    note "update check failed: could not download the latest signed release list from GitHub (still on $HAVE)"; exit 1
  fi
  if ! LATEST=$(verify_release "$REL"); then
    note "update refused: the latest release list is not signed by a pinned release key (still on $HAVE)"; exit 1
  fi
  if ! newer "$LATEST" "$HAVE"; then
    note "up to date ($HAVE; latest signed release $LATEST)"; exit 0
  fi
  # A restart ends every standing approval (their use keys live only in memory).
  # Wait while any is active, for up to 7 days after the first wait on this
  # installed version (a newer release does not restart the clock). Waiting
  # requests do not count: an agent can make one at any time.
  if [ "$FORCE" != yes ]; then
    st=$(curl -q --max-time 5 -fsS "http://localhost:$PORT/api/status" 2>/dev/null || true)
    a=$(printf '%s' "$st" | sed -n 's/.*"standing_active": *\([0-9][0-9]*\).*/\1/p')
    if [ "${a:-0}" -gt 0 ]; then
      since=$(awk -v v="$HAVE" '$2 == v && $1 ~ /^[0-9]+$/ { print $1 }' "$PREFIX/update-postponed" 2>/dev/null || true)
      nowts=$(date +%s)
      if [ -z "$since" ]; then
        since=$nowts
        printf '%s %s\n' "$since" "$HAVE" > "$PREFIX/update-postponed.new" && mv -f "$PREFIX/update-postponed.new" "$PREFIX/update-postponed"
      fi
      if [ $(( nowts - since )) -lt $(( 7 * 86400 )) ]; then
        note "waiting to install $LATEST: ${a} standing approval(s) active. It installs once they end, after 7 days at the latest, or now with: $P_BIN-admin update --now"
        exit 0
      fi
    fi
  fi
  rm -f "$PREFIX/update-postponed"
  say "Updating from $HAVE to the signed release $LATEST"
  VERSION="$LATEST"
fi

# A Phygital Key 0.3 install (one program for everything) shares the parent folders
# and the port, and its uninstall removes /Library/PhygitalKey. Remove it first.
if [ "$P_BIN" != phygital-key ]; then
  for old in /Library/PhygitalKey/bin/phygital-key /Library/LaunchDaemons/io.phygitalkey.gate.plist \
             /usr/lib/phygital-key/bin/phygital-key /etc/systemd/system/phygital-key.service; do
    [ -e "$old" ] && die "an older Phygital Key 0.3 secure install is here ($old). Remove it first: sudo /Library/PhygitalKey/bin/phygital-key install-service --uninstall --purge (Linux: sudo /usr/lib/phygital-key/bin/phygital-key install-service --uninstall --purge)"
  done
fi

# 1. Get the program: the official signed release, checked, or a binary root already owns.
if [ -n "$BINARY" ]; then
  root_only "$BINARY" || die "$BINARY is not safe to run as root: it (or a folder above it) is not owned by root only."
  cp "$BINARY" "$WORK/program"
else
  case "$(uname -m)" in arm64|aarch64) ARCH=aarch64 ;; x86_64|amd64) ARCH=x86_64 ;; *) die "unsupported CPU $(uname -m)" ;; esac
  NAME="$P_BIN-$VERSION-$ARCH-$TRIPLE_OS"
  BASE="https://github.com/$REPO/releases/download/$VERSION"
  URL="$BASE/$NAME.tar.gz"
  if [ "$AUTO_RUN" != yes ]; then
    fetch_release_list "$BASE" "$REL" || die "could not download the signed release list for $VERSION."
  fi
  got=$(verify_release "$REL") || die "the release list for $VERSION is not signed by a pinned release key. Nothing was installed."
  [ "$got" = "$VERSION" ] || die "the signed release list is for $got, not $VERSION. Nothing was installed."
  want=$(awk -v n="$NAME.tar.gz" '$2 == n && $1 ~ /^[0-9a-f]+$/ && length($1) == 64 { print $1 }' "$REL/release.txt")
  [ -n "$want" ] || die "the signed release list has no $NAME.tar.gz. Nothing was installed."
  say "Downloading the official $VERSION release for this machine (signed release list checked)"
  curl -q --proto '=https' --tlsv1.2 --max-time 300 -fsSL "$URL" -o "$WORK/r.tar.gz" || die "download failed: $URL"
  if command -v sha256sum >/dev/null; then have=$(sha256sum "$WORK/r.tar.gz" | cut -d ' ' -f 1)
  else have=$(shasum -a 256 "$WORK/r.tar.gz" | cut -d ' ' -f 1); fi
  [ "$want" = "$have" ] || die "the download does not match its signed SHA-256. Nothing was installed."
  tar --no-same-owner -xzf "$WORK/r.tar.gz" -C "$WORK"
  [ -f "$WORK/$NAME/$P_BIN" ] && [ ! -L "$WORK/$NAME/$P_BIN" ] || die "the release does not contain the expected program."
  mv "$WORK/$NAME/$P_BIN" "$WORK/program"
fi

# 2. The gate user: no login, no shell, no home.
if [ "$OS" = mac ]; then
  if ! dscl . -read "/Users/$SVC_USER" >/dev/null 2>&1; then
    id=""
    for n in $(seq 400 499); do
      if ! dscl . -list /Users UniqueID | awk '{print $2}' | grep -qx "$n" &&
         ! dscl . -list /Groups PrimaryGroupID | awk '{print $2}' | grep -qx "$n"; then id=$n; break; fi
    done
    [ -n "$id" ] || die "no free system user id between 400 and 499."
    dscl . -create "/Groups/$SVC_USER" PrimaryGroupID "$id"
    dscl . -create "/Groups/$SVC_USER" RealName "$P_NAME"
    dscl . -create "/Users/$SVC_USER"
    dscl . -create "/Users/$SVC_USER" UniqueID "$id"
    dscl . -create "/Users/$SVC_USER" PrimaryGroupID "$id"
    dscl . -create "/Users/$SVC_USER" UserShell /usr/bin/false
    dscl . -create "/Users/$SVC_USER" NFSHomeDirectory /var/empty
    dscl . -create "/Users/$SVC_USER" RealName "$P_NAME"
    dscl . -create "/Users/$SVC_USER" IsHidden 1
  fi
else
  id -u "$SVC_USER" >/dev/null 2>&1 || useradd --system --no-create-home --home-dir /nonexistent --shell /usr/sbin/nologin "$SVC_USER"
fi

# 3. Folders: program and helper only root can change; data only the gate user can open.
#    Every folder above them must be root's alone: a folder another user owns could
#    be renamed or swapped (for example for a link where root writes a log).
for d in "$(dirname "$PREFIX")" "$(dirname "$DATA")" ${LOGS:+"$(dirname "$LOGS")"} ${UPD_LOGS:+"$(dirname "$UPD_LOGS")"}; do
  if [ -e "$d" ] && ! root_only "$d"; then
    die "$d is not owned by root only (perhaps left by an older install). Refusing to continue."
  fi
  install -d -m 0755 -o root "$d"
  root_only "$d" || die "$d is not owned by root only. Refusing to continue."
done
install -d -m 0755 -o root "$PREFIX" "$PREFIX/bin"
root_only "$PREFIX/bin" || die "$PREFIX/bin is not owned by root only. Refusing to continue."
install -d -m 0700 -o "$SVC_USER" "$DATA"
# macOS: launchd opens the log as the gate user, so the folder is the gate's own.
# Linux: no log file at all; output goes to the system journal.
[ -n "$LOGS" ] && install -d -m 0700 -o "$SVC_USER" "$LOGS"

# 4. The program, swapped in at once (no moment without a gate binary). The old
#    one is kept until the new gate has passed its checks, to switch back to.
[ -f "$BIN" ] && cp -p "$BIN" "$WORK/previous"
install -m 0755 -o root "$WORK/program" "$BIN.new"
mv -f "$BIN.new" "$BIN"

# 5. The admin helper. Every command goes through sudo, so an agent acting as you
#    cannot enroll a key or read the gate's files without your password.
cat > "$WORK/admin" <<ADMINSCRIPT
#!/bin/bash
# $P_NAME admin helper (installed by install-service). Run it by its full path
# if /usr/local/bin is not root-only on this machine: $ADMIN
set -euo pipefail
# Only system folders: sudo below must never run a program from a folder you can write.
export PATH=/usr/bin:/bin:/usr/sbin:/sbin
run() { sudo -u "$SVC_USER" "$BIN" --home "$DATA" --port "$PORT" "\$@"; }
listeners_ok() {
  local pids
  if command -v lsof >/dev/null 2>&1; then pids=\$(sudo lsof -nP -iTCP:$PORT -sTCP:LISTEN -t 2>/dev/null | sort -u)
  else pids=\$(sudo ss -ltnpH "sport = :$PORT" 2>/dev/null | grep -o 'pid=[0-9]*' | cut -d= -f2 | sort -u); fi
  [ -n "\$pids" ] || return 1
  local want; want=\$(id -u "$SVC_USER")
  for p in \$pids; do [ "\$(ps -o uid= -p "\$p" | tr -d ' ')" = "\$want" ] || return 1; done
}
case "\${1:-help}" in
  enroll) run enroll ;;
  token) run token ;;
  keys) run keys ;;
  audit-check) run audit-check ;;
  update) shift; if [ "\${1:-}" = --now ]; then sudo "$BIN" --port "$PORT" install-service --auto-update-run --now; else sudo "$BIN" --port "$PORT" install-service --auto-update-run; fi ;;
  status)
    echo "program:      \$("$BIN" --version)"
    echo "auto-update:  \$(cat "$PREFIX/auto-update" 2>/dev/null || echo on)  (last: \$(cat "$PREFIX/update-status" 2>/dev/null || echo "not run yet"))"
    if listeners_ok; then echo "ok: only the gate ($SVC_USER) listens on port $PORT"; curl -q --max-time 5 -fsS "http://localhost:$PORT/api/status"; echo
    else echo "WARNING: something other than the gate is listening on port $PORT, or the gate is not running. Do not approve anything until this is fixed."; exit 1; fi ;;
  logs) if [ -n "$LOGS" ]; then sudo tail -n 50 "$LOGS/gate.log"; else sudo journalctl -u $P_BIN -n 50 --no-pager; fi ;;
  *) echo "usage: $P_BIN-admin enroll | token | keys | audit-check | status | logs | update [--now]" ;;
esac
ADMINSCRIPT
install -m 0755 -o root "$WORK/admin" "$ADMIN"
if [ -d /usr/local/bin ] && root_only /usr/local/bin 2>/dev/null; then
  ln -sfn "$ADMIN" "$LINK"
  HELPER="$P_BIN-admin"
else
  HELPER="$ADMIN"
fi

refuse() { stop_service; die "the gate is stopped: $1"; }
SVC_UID=$(id -u "$SVC_USER")
# Refuse the moment a program other than the gate listens on the port.
foreign_listener() {
  local p u
  for p in $(listener_pids "$PORT" || true); do
    u=$(ps -o uid= -p "$p" | tr -d ' ')
    [ "$u" = "$SVC_UID" ] || { echo "uid $u, pid $p"; return 0; }
  done
  return 1
}
foreign_pids() {
  local p u
  for p in $(listener_pids "$PORT" || true); do
    u=$(ps -o uid= -p "$p" | tr -d ' ')
    [ "$u" = "$SVC_UID" ] || echo "$p"
  done
}
KILLED=""
# Before starting anything: nothing else may already hold the gate's port.
if who=$(foreign_listener); then
  refuse "another program ($who) is listening on port $PORT. Stop it, then run this again."
fi

# 6. The service (written and started; used again to switch back on a failed update).
start_gate() {
if [ "$OS" = mac ]; then
  cat > "$WORK/plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>$LABEL.gate</string>
  <key>UserName</key><string>$SVC_USER</string>
  <key>GroupName</key><string>$SVC_USER</string>
  <key>ProgramArguments</key>
  <array>
    <string>$BIN</string><string>--home</string><string>$DATA</string><string>--port</string><string>$PORT</string><string>serve</string>
  </array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
  <key>SoftResourceLimits</key>
  <dict><key>NumberOfFiles</key><integer>8192</integer></dict>
  <key>Umask</key><integer>63</integer>
  <key>StandardOutPath</key><string>$LOGS/gate.log</string>
  <key>StandardErrorPath</key><string>$LOGS/gate.log</string>
</dict>
</plist>
PLIST
  install -m 0644 -o root "$WORK/plist" "$UNIT"
  launchctl bootout "system/$LABEL.gate" 2>/dev/null || true
  # Wait until an old instance has really let go of the port.
  for _ in $(seq 1 50); do [ -z "$(listener_pids "$PORT")" ] && break; sleep 0.2; done
  ok=no
  for _ in 1 2 3 4 5 6 7 8 9 10; do
    if launchctl bootstrap system "$UNIT" 2>/dev/null; then ok=yes; break; fi
    sleep 0.5   # bootout may still be finishing
  done
  [ "$ok" = yes ] || say "could not start the service (launchctl bootstrap)."
else
  cat > "$WORK/unit" <<UNITFILE
[Unit]
Description=$P_NAME approval gate
After=network.target

[Service]
User=$SVC_USER
Group=$SVC_USER
ExecStart=$BIN --home $DATA --port $PORT serve
Restart=on-failure
UMask=0077
LimitNOFILE=8192
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=yes
ProtectProc=invisible
PrivateTmp=yes
PrivateDevices=yes
ProtectClock=yes
ProtectKernelTunables=yes
ProtectKernelModules=yes
ProtectKernelLogs=yes
ProtectControlGroups=yes
ProtectHostname=yes
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX
RestrictNamespaces=yes
RestrictRealtime=yes
RestrictSUIDSGID=yes
LockPersonality=yes
MemoryDenyWriteExecute=yes
SystemCallArchitectures=native
SystemCallFilter=@system-service
CapabilityBoundingSet=
IPAddressDeny=any
IPAddressAllow=localhost
ReadWritePaths=$DATA
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
UNITFILE
  install -m 0644 -o root "$WORK/unit" "$UNIT"
  systemctl daemon-reload
  systemctl enable "$P_BIN.service" >/dev/null 2>&1
  systemctl stop "$P_BIN.service" 2>/dev/null || true
  for _ in $(seq 1 50); do [ -z "$(listener_pids "$PORT")" ] && break; sleep 0.2; done
  systemctl start "$P_BIN.service" || true
fi
}

# 7. Check that it answers, and that ONLY the gate listens on its port
#    (another program on the same port could show a fake approval page).
#    If not, stop the service again: never leave a half-trusted gate running.
gate_up() {
  local who tries=0
  for _ in $(seq 1 150); do
    if who=$(foreign_listener); then
      if [ "$AUTO_RUN" = yes ] && [ "$tries" -lt 5 ]; then
        # Unattended update: a program that took the gate's port while the gate
        # restarted is posing as the gate. Never leave it the address: stop it and
        # start the gate again.
        tries=$((tries + 1))
        for p in $(foreign_pids); do kill -9 "$p" 2>/dev/null || true; done
        KILLED="$KILLED [$who]"
        start_gate
        continue
      fi
      refuse "another program ($who) is listening on port $PORT. Stop it, then run this again. Do not approve anything until then."
    fi
    if [ -n "$(listener_pids "$PORT" || true)" ] && curl -q --max-time 2 -fsS -o /dev/null "http://localhost:$PORT/api/status" 2>/dev/null; then
      if who=$(foreign_listener); then
        refuse "another program ($who) is listening on port $PORT. Stop it, then run this again. Do not approve anything until then."
      fi
      return 0
    fi
    sleep 0.2
  done
  return 1
}

start_gate
if ! gate_up; then
  if [ -f "$WORK/previous" ]; then
    say "The new gate did not start. Switching back to the previous program ($HAVE)."
    install -m 0755 -o root "$WORK/previous" "$BIN.new"
    mv -f "$BIN.new" "$BIN"
    start_gate
    if gate_up; then
      note "update to $VERSION failed: the new gate did not start, so $HAVE is back"
      exit 1
    fi
  fi
  refuse "the gate did not start. See: $ADMIN logs. To install again: sudo $BIN install-service"
fi
if [ "$AUTO_RUN" = yes ]; then
  note "updated from $HAVE to $VERSION (signed release, SHA-256 checked)${KILLED:+; stopped programs that took the port of the gate during the restart:$KILLED}"
  exit 0
fi

# 8. Daily updates to the latest signed release (on unless turned off). Only
#    releases newer than the installed one, whose release list is signed by a
#    pinned release key, are installed; a gate that fails its checks is switched
#    back to the previous program.
AUTO=$(cat "$PREFIX/auto-update" 2>/dev/null || echo on)
[ -n "$AUTO_PREF" ] && AUTO="$AUTO_PREF"
case "$AUTO" in on|off) ;; *) AUTO=on ;; esac
printf '%s\n' "$AUTO" > "$WORK/auto"
install -m 0644 -o root "$WORK/auto" "$PREFIX/auto-update"
stop_updates
if [ "$AUTO" = on ]; then
  if [ "$OS" = mac ]; then
    install -d -m 0700 -o root "$UPD_LOGS"
    H=$((RANDOM % 24)); M=$((RANDOM % 60))
    cat > "$WORK/upd" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>$LABEL.update</string>
  <key>ProgramArguments</key>
  <array>
    <string>$BIN</string><string>--port</string><string>$PORT</string><string>install-service</string><string>--auto-update-run</string>
  </array>
  <key>StartCalendarInterval</key>
  <dict><key>Hour</key><integer>$H</integer><key>Minute</key><integer>$M</integer></dict>
  <key>EnvironmentVariables</key>
  <dict><key>HOME</key><string>/var/root</string></dict>
  <key>Umask</key><integer>18</integer>
  <key>StandardOutPath</key><string>$UPD_LOGS/update.log</string>
  <key>StandardErrorPath</key><string>$UPD_LOGS/update.log</string>
</dict>
</plist>
PLIST
    install -m 0600 -o root "$WORK/upd" "$UPD_UNIT"
    launchctl bootstrap system "$UPD_UNIT" 2>/dev/null || say "Note: could not start the daily update job (launchctl). Updates: sudo $BIN install-service --release vX.Y.Z"
    UPDATES="daily at $(printf '%02d:%02d' "$H" "$M"), signed releases only (off: sudo $BIN install-service --no-auto-update)"
  else
    cat > "$WORK/upd" <<UNITFILE
[Unit]
Description=$P_NAME daily update (signed releases only)
After=network-online.target
Wants=network-online.target

[Service]
Type=oneshot
ExecStart=$BIN --port $PORT install-service --auto-update-run
# systemd gives a service no HOME, and the program needs one to start.
Environment=HOME=/root
# Root, because it replaces the program and restarts the gate, so the files it
# may write stay open. Everything else it never needs is closed.
PrivateTmp=yes
ProtectHome=yes
NoNewPrivileges=yes
PrivateDevices=yes
ProtectClock=yes
ProtectKernelTunables=yes
ProtectKernelModules=yes
ProtectKernelLogs=yes
ProtectControlGroups=yes
ProtectHostname=yes
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX
RestrictNamespaces=yes
RestrictRealtime=yes
RestrictSUIDSGID=yes
LockPersonality=yes
SystemCallArchitectures=native
UNITFILE
    cat > "$WORK/timer" <<UNITFILE
[Unit]
Description=$P_NAME daily update check

[Timer]
OnCalendar=daily
RandomizedDelaySec=6h
Persistent=true

[Install]
WantedBy=timers.target
UNITFILE
    install -m 0644 -o root "$WORK/upd" "$UPD_UNIT"
    install -m 0644 -o root "$WORK/timer" "$UPD_TIMER"
    systemctl daemon-reload
    systemctl enable --now "$P_BIN-update.timer" >/dev/null 2>&1 || say "Note: could not start the daily update timer (systemctl)."
    UPDATES="daily, signed releases only (off: sudo $BIN install-service --no-auto-update)"
  fi
else
  UPDATES="off (on again: sudo $BIN install-service --auto-update; now: $HELPER update)"
fi

cat <<DONE
$P_NAME is installed as a locked-down service.
  approval page   http://localhost:$PORT
  runs as         $SVC_USER: you, and any AI acting as you, cannot change its program,
                  read its data or write its keys without your administrator password
  program         $BIN ($([ -n "$BINARY" ] && echo "from $BINARY" || echo "official $VERSION release, signed list and SHA-256 checked"))
  updates         $UPDATES

Next:
  1. Add your key:        $HELPER enroll
  2. Connect your AI app: $HELPER token    (prints the line to paste)
  3. Check it any time:   $HELPER status
Update now:    $HELPER update   (or a chosen version: sudo $BIN install-service --release vX.Y.Z)
Remove:        sudo $BIN install-service --uninstall
DONE
