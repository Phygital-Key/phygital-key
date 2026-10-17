#!/bin/bash
# Phygital Key: run the gate as its own locked-down system user (macOS, Linux).
#
# Recommended (from a fresh terminal you trust):
#   curl -fsSL https://raw.githubusercontent.com/s17sh70x2jfl031ag/phygital-key-mcp/v0.3.0/crates/phygital-key/assets/install-service.sh | sudo bash
# Later, use the locked copy it installs (add --release vX.Y.Z to update):
#   sudo /Library/PhygitalKey/bin/phygital-key install-service [--release vX.Y.Z | --uninstall]   (macOS)
#   sudo /usr/lib/phygital-key/bin/phygital-key install-service [--release vX.Y.Z | --uninstall]  (Linux)
# Shortcut: `sudo phygital-key install-service` runs YOUR copy of the program as
# administrator. Only use it if nothing else could have changed that file.
#
# Options:  --uninstall   remove the service (keeps keys and log)
#           --purge       with --uninstall: also delete the gate user, keys and log
#
# Why: an AI agent runs as you. If the gate also ran as you, the agent could edit
# the gate's program, keys and log. Here the gate runs as a separate user. Its
# program is the official release, checked against its published SHA-256 and kept
# in folders only an administrator can change, and its data folder is closed to
# everyone else. Nothing that you (or an agent acting as you) could have edited is
# ever run as root or installed.
set -euo pipefail

REPO="s17sh70x2jfl031ag/phygital-key-mcp"
VERSION="${PKA_VERSION:-v0.3.0}"
PORT="${PKA_PORT:-7468}"
BINARY="${PKA_BINARY:-}"   # optional: an already root-owned binary (used by CI)
MODE=install
PURGE=no
for a in "$@"; do
  case "$a" in
    --uninstall) MODE=uninstall ;;
    --purge) PURGE=yes ;;
    *) echo "unknown option: $a" >&2; exit 2 ;;
  esac
done
say() { printf '%s\n' "$*"; }
die() { printf 'phygital-key install-service: %s\n' "$*" >&2; exit 1; }
[ "$(id -u)" = 0 ] || die "needs administrator rights (sudo)."
[[ "$VERSION" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "bad version: $VERSION"
[[ "$PORT" =~ ^[0-9]{1,5}$ ]] && [ "$PORT" -ge 1 ] && [ "$PORT" -le 65535 ] || die "bad port: $PORT"
case "$BINARY" in ""|/*) ;; *) die "--binary needs a full path (starting with /)" ;; esac

case "$(uname -s)" in
  Darwin)
    OS=mac; SVC_USER=_phygitalkey
    PREFIX=/Library/PhygitalKey
    DATA="/Library/Application Support/PhygitalKey"
    LOGS=/Library/Logs/PhygitalKey
    UNIT=/Library/LaunchDaemons/io.phygitalkey.gate.plist
    TRIPLE_OS=apple-darwin ;;
  Linux)
    OS=linux; SVC_USER=phygitalkey
    PREFIX=/usr/lib/phygital-key
    DATA=/var/lib/phygital-key
    LOGS=""
    UNIT=/etc/systemd/system/phygital-key.service
    TRIPLE_OS=unknown-linux-gnu ;;
  *) die "only macOS and Linux for now." ;;
esac
BIN="$PREFIX/bin/phygital-key"
ADMIN="$PREFIX/bin/phygital-key-admin"
LINK=/usr/local/bin/phygital-key-admin

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

stop_service() {
  if [ "$OS" = mac ]; then launchctl bootout system/io.phygitalkey.gate 2>/dev/null || true
  else systemctl disable --now phygital-key.service 2>/dev/null || true; fi
}

if [ "$MODE" = uninstall ]; then
  stop_service
  rm -f "$UNIT"
  [ -L "$LINK" ] && [ "$(readlink "$LINK")" = "$ADMIN" ] && rm -f "$LINK"
  rm -rf "$PREFIX"
  [ "$OS" = linux ] && systemctl daemon-reload || true
  if [ "$PURGE" = yes ]; then
    rm -rf "$DATA"; [ -n "$LOGS" ] && rm -rf "$LOGS"
    if [ "$OS" = mac ]; then
      dscl . -delete "/Users/$SVC_USER" 2>/dev/null || true
      dscl . -delete "/Groups/$SVC_USER" 2>/dev/null || true
    else userdel "$SVC_USER" 2>/dev/null || true; fi
    say "Removed Phygital Key, its user, keys and log."
  else
    say "Removed the Phygital Key service. Your keys and log are kept in: $DATA"
  fi
  exit 0
fi

# 1. Get the program: the official release, checked, or a binary root already owns.
WORK=$(mktemp -d /tmp/phygital-key-install.XXXXXX)
chmod 700 "$WORK"
trap 'rm -rf "$WORK"' EXIT
if [ -n "$BINARY" ]; then
  root_only "$BINARY" || die "$BINARY is not safe to run as root: it (or a folder above it) is not owned by root only."
  cp "$BINARY" "$WORK/phygital-key"
else
  case "$(uname -m)" in arm64|aarch64) ARCH=aarch64 ;; x86_64|amd64) ARCH=x86_64 ;; *) die "unsupported CPU $(uname -m)" ;; esac
  NAME="phygital-key-$VERSION-$ARCH-$TRIPLE_OS"
  URL="https://github.com/$REPO/releases/download/$VERSION/$NAME.tar.gz"
  say "Downloading the official $VERSION release for this machine"
  curl --proto '=https' --tlsv1.2 -fsSL "$URL" -o "$WORK/r.tar.gz" || die "download failed: $URL"
  curl --proto '=https' --tlsv1.2 -fsSL "$URL.sha256" -o "$WORK/r.sha256" || die "checksum download failed"
  want=$(cut -d ' ' -f 1 < "$WORK/r.sha256")
  if command -v sha256sum >/dev/null; then have=$(sha256sum "$WORK/r.tar.gz" | cut -d ' ' -f 1)
  else have=$(shasum -a 256 "$WORK/r.tar.gz" | cut -d ' ' -f 1); fi
  [ "$want" = "$have" ] || die "the download does not match its published SHA-256. Nothing was installed."
  tar --no-same-owner -xzf "$WORK/r.tar.gz" -C "$WORK"
  [ -f "$WORK/$NAME/phygital-key" ] && [ ! -L "$WORK/$NAME/phygital-key" ] || die "the release does not contain the expected program."
  mv "$WORK/$NAME/phygital-key" "$WORK/phygital-key"
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
    dscl . -create "/Groups/$SVC_USER" RealName "Phygital Key"
    dscl . -create "/Users/$SVC_USER"
    dscl . -create "/Users/$SVC_USER" UniqueID "$id"
    dscl . -create "/Users/$SVC_USER" PrimaryGroupID "$id"
    dscl . -create "/Users/$SVC_USER" UserShell /usr/bin/false
    dscl . -create "/Users/$SVC_USER" NFSHomeDirectory /var/empty
    dscl . -create "/Users/$SVC_USER" RealName "Phygital Key"
    dscl . -create "/Users/$SVC_USER" IsHidden 1
  fi
else
  id -u "$SVC_USER" >/dev/null 2>&1 || useradd --system --no-create-home --home-dir /nonexistent --shell /usr/sbin/nologin "$SVC_USER"
fi

# 3. Folders: program and helper only root can change; data only the gate user can open.
install -d -m 0755 -o root "$PREFIX" "$PREFIX/bin"
root_only "$PREFIX/bin" || die "$PREFIX/bin is not owned by root only. Refusing to continue."
install -d -m 0700 -o "$SVC_USER" "$DATA"
# macOS: launchd opens the log as the gate user, so the folder is the gate's own.
# Linux: no log file at all; output goes to the system journal.
[ -n "$LOGS" ] && install -d -m 0700 -o "$SVC_USER" "$LOGS"

# 4. The program, swapped in at once (no moment without a gate binary).
install -m 0755 -o root "$WORK/phygital-key" "$BIN.new"
mv -f "$BIN.new" "$BIN"

# 5. The admin helper. Every command goes through sudo, so an agent acting as you
#    cannot enroll a key or read the gate's files without your password.
cat > "$WORK/admin" <<ADMINSCRIPT
#!/bin/bash
# Phygital Key admin helper (installed by install-service). Run it by its full path
# if /usr/local/bin is not root-only on this machine: $ADMIN
set -euo pipefail
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
  token) run mcp-token ;;
  keys) run keys ;;
  audit-check) run audit-check ;;
  status)
    if listeners_ok; then echo "ok: only the gate ($SVC_USER) listens on port $PORT"; curl --max-time 5 -fsS "http://localhost:$PORT/api/status"; echo
    else echo "WARNING: something other than the gate is listening on port $PORT, or the gate is not running. Do not approve anything until this is fixed."; exit 1; fi ;;
  logs) if [ -n "$LOGS" ]; then sudo tail -n 50 "$LOGS/gate.log"; else sudo journalctl -u phygital-key -n 50 --no-pager; fi ;;
  *) echo "usage: phygital-key-admin enroll | token | keys | audit-check | status | logs" ;;
esac
ADMINSCRIPT
install -m 0755 -o root "$WORK/admin" "$ADMIN"
if [ -d /usr/local/bin ] && root_only /usr/local/bin 2>/dev/null; then
  ln -sfn "$ADMIN" "$LINK"
  HELPER=phygital-key-admin
else
  HELPER="$ADMIN"
fi

refuse() { stop_service; die "$1"; }
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
# Before starting anything: nothing else may already hold the gate's port.
if who=$(foreign_listener); then
  refuse "another program ($who) is listening on port $PORT. Stop it, then run this again."
fi

# 6. The service.
if [ "$OS" = mac ]; then
  cat > "$WORK/plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>io.phygitalkey.gate</string>
  <key>UserName</key><string>$SVC_USER</string>
  <key>GroupName</key><string>$SVC_USER</string>
  <key>ProgramArguments</key>
  <array>
    <string>$BIN</string><string>--home</string><string>$DATA</string><string>--port</string><string>$PORT</string><string>serve</string>
  </array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
  <key>Umask</key><integer>63</integer>
  <key>StandardOutPath</key><string>$LOGS/gate.log</string>
  <key>StandardErrorPath</key><string>$LOGS/gate.log</string>
</dict>
</plist>
PLIST
  install -m 0644 -o root "$WORK/plist" "$UNIT"
  launchctl bootout system/io.phygitalkey.gate 2>/dev/null || true
  # Wait until an old instance has really let go of the port.
  for _ in $(seq 1 50); do [ -z "$(listener_pids "$PORT")" ] && break; sleep 0.2; done
  ok=no
  for _ in 1 2 3 4 5 6 7 8 9 10; do
    if launchctl bootstrap system "$UNIT" 2>/dev/null; then ok=yes; break; fi
    sleep 0.5   # bootout may still be finishing
  done
  [ "$ok" = yes ] || die "could not start the service (launchctl bootstrap)."
else
  cat > "$WORK/unit" <<UNITFILE
[Unit]
Description=Phygital Key approval gate
After=network.target

[Service]
User=$SVC_USER
Group=$SVC_USER
ExecStart=$BIN --home $DATA --port $PORT serve
Restart=on-failure
UMask=0077
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
  systemctl enable phygital-key.service >/dev/null 2>&1
  systemctl stop phygital-key.service 2>/dev/null || true
  for _ in $(seq 1 50); do [ -z "$(listener_pids "$PORT")" ] && break; sleep 0.2; done
  systemctl start phygital-key.service
fi

# 7. Check that it answers, and that ONLY the gate listens on its port
#    (another program on the same port could show a fake approval page).
#    If not, stop the service again: never leave a half-trusted gate running.
up=no
for _ in $(seq 1 150); do
  if who=$(foreign_listener); then
    refuse "another program ($who) is listening on port $PORT. Stop it, then run this again. Do not approve anything until then."
  fi
  if [ -n "$(listener_pids "$PORT" || true)" ] && curl --max-time 2 -fsS -o /dev/null "http://localhost:$PORT/api/status" 2>/dev/null; then
    up=yes; break
  fi
  sleep 0.2
done
[ "$up" = yes ] || refuse "the gate did not start. See: $ADMIN logs"
if who=$(foreign_listener); then
  refuse "another program ($who) is listening on port $PORT. Stop it, then run this again. Do not approve anything until then."
fi

cat <<DONE
Phygital Key is installed as a locked-down service.
  approval page   http://localhost:$PORT
  runs as         $SVC_USER: you, and any AI acting as you, cannot change its program,
                  read its data or write its keys without your administrator password
  program         $BIN ($([ -n "$BINARY" ] && echo "from $BINARY" || echo "official $VERSION release, SHA-256 checked"))

Next:
  1. Add your key:        $HELPER enroll
  2. Connect your AI app: $HELPER token    (prints the line to paste)
  3. Check it any time:   $HELPER status
Update later:  sudo $BIN install-service --release vX.Y.Z
Remove:        sudo $BIN install-service --uninstall
DONE
