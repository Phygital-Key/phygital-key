#!/bin/bash
# Phygital Key: run the gate as its own locked-down system user.
#
#   sudo phygital-key install-service              install or update
#   sudo phygital-key install-service --uninstall  remove (keeps your keys and log)
#   sudo phygital-key install-service --uninstall --purge   remove everything
#
# Why: an AI agent runs as you. If the gate also ran as you, the agent could
# edit the gate's files (its program, keyring, audit log). Here the gate runs
# as a separate user whose files you (and the agent) cannot change without
# your administrator password.
#
# What it creates (macOS / Linux):
#   user          _phygitalkey (hidden)           / phygitalkey (system user)
#   program       /usr/local/libexec/phygital-key/phygital-key   (owned by root)
#   data          /Library/Application Support/PhygitalKey  / /var/lib/phygital-key  (0700, gate user)
#   service       /Library/LaunchDaemons/io.phygitalkey.gate.plist  / /etc/systemd/system/phygital-key.service
#   admin helper  /usr/local/bin/phygital-key-admin   (enroll, token, keys, status, logs)
set -euo pipefail

SELF="${PKA_SELF:?run this through: sudo phygital-key install-service}"
PORT="${PKA_PORT:-7468}"
MODE=install
PURGE=no
for a in "$@"; do
  case "$a" in
    --uninstall) MODE=uninstall ;;
    --purge) PURGE=yes ;;
    *) echo "unknown option: $a" >&2; exit 2 ;;
  esac
done

[ "$(id -u)" = 0 ] || { echo "Needs administrator rights: sudo phygital-key install-service" >&2; exit 1; }

LIBEXEC=/usr/local/libexec/phygital-key
BIN="$LIBEXEC/phygital-key"
ADMIN=/usr/local/bin/phygital-key-admin

case "$(uname -s)" in
  Darwin)
    OS=mac
    SVC_USER=_phygitalkey
    DATA="/Library/Application Support/PhygitalKey"
    LOGS=/Library/Logs/PhygitalKey
    UNIT=/Library/LaunchDaemons/io.phygitalkey.gate.plist
    ;;
  Linux)
    OS=linux
    SVC_USER=phygitalkey
    DATA=/var/lib/phygital-key
    LOGS=/var/log/phygital-key
    UNIT=/etc/systemd/system/phygital-key.service
    ;;
  *) echo "Only macOS and Linux for now." >&2; exit 1 ;;
esac

stop_service() {
  if [ "$OS" = mac ]; then
    launchctl bootout system/io.phygitalkey.gate 2>/dev/null || true
  else
    systemctl disable --now phygital-key.service 2>/dev/null || true
  fi
}

if [ "$MODE" = uninstall ]; then
  stop_service
  rm -f "$UNIT" "$ADMIN"
  rm -rf "$LIBEXEC"
  [ "$OS" = linux ] && systemctl daemon-reload || true
  if [ "$PURGE" = yes ]; then
    rm -rf "$DATA" "$LOGS"
    if [ "$OS" = mac ]; then
      dscl . -delete "/Users/$SVC_USER" 2>/dev/null || true
      dscl . -delete "/Groups/$SVC_USER" 2>/dev/null || true
    else
      userdel "$SVC_USER" 2>/dev/null || true
    fi
    echo "Removed Phygital Key, its user, keys and log."
  else
    echo "Removed the Phygital Key service. Your keys and log are kept in: $DATA"
  fi
  exit 0
fi

# 1. The service user (no login, no shell, no home).
if [ "$OS" = mac ]; then
  if ! dscl . -read "/Users/$SVC_USER" >/dev/null 2>&1; then
    id=""
    for n in $(seq 400 499); do
      if ! dscl . -list /Users UniqueID | awk '{print $2}' | grep -qx "$n" &&
         ! dscl . -list /Groups PrimaryGroupID | awk '{print $2}' | grep -qx "$n"; then id=$n; break; fi
    done
    [ -n "$id" ] || { echo "No free system user id between 400 and 499." >&2; exit 1; }
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

# 2. The program, owned by root: nobody else can replace it.
stop_service
install -d -m 0755 -o root "$LIBEXEC"
install -m 0755 -o root "$SELF" "$BIN"

# 3. The data folder, only the service user can read or write it.
install -d -m 0700 -o "$SVC_USER" "$DATA"
install -d -m 0750 -o "$SVC_USER" "$LOGS"

# 4. The service.
if [ "$OS" = mac ]; then
  cat > "$UNIT" <<PLIST
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
  chown root:wheel "$UNIT"
  chmod 0644 "$UNIT"
  launchctl bootstrap system "$UNIT"
else
  cat > "$UNIT" <<UNITFILE
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
PrivateTmp=yes
PrivateDevices=yes
ProtectKernelTunables=yes
ProtectKernelModules=yes
ProtectControlGroups=yes
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX
RestrictNamespaces=yes
LockPersonality=yes
MemoryDenyWriteExecute=yes
CapabilityBoundingSet=
ReadWritePaths=$DATA
StandardOutput=append:$LOGS/gate.log
StandardError=append:$LOGS/gate.log

[Install]
WantedBy=multi-user.target
UNITFILE
  chmod 0644 "$UNIT"
  systemctl daemon-reload
  systemctl enable --now phygital-key.service
fi

# 5. A small admin helper. Each command asks for your administrator password,
#    so an AI acting as you cannot enroll keys or read the MCP token by itself.
cat > "$ADMIN" <<ADMINSCRIPT
#!/bin/bash
# Phygital Key admin helper (installed by: sudo phygital-key install-service).
set -euo pipefail
run() { sudo -u "$SVC_USER" "$BIN" --home "$DATA" --port "$PORT" "\$@"; }
case "\${1:-help}" in
  enroll) run enroll ;;
  token) run mcp-token ;;
  keys) run keys ;;
  audit-check) run audit-check ;;
  status) curl -fsS "http://localhost:$PORT/api/status" && echo ;;
  logs) sudo tail -n 50 "$LOGS/gate.log" ;;
  *) echo "usage: phygital-key-admin enroll | token | keys | audit-check | status | logs" ;;
esac
ADMINSCRIPT
chmod 0755 "$ADMIN"
chown root "$ADMIN"

# 6. Wait for it to answer.
for _ in $(seq 1 50); do
  curl -fsS -o /dev/null "http://localhost:$PORT/api/status" 2>/dev/null && break
  sleep 0.2
done

cat <<DONE
Phygital Key is installed as a locked-down service.
  approval page   http://localhost:$PORT
  runs as         $SVC_USER (you cannot change its files without your password)
  data            $DATA

Next:
  1. Add your key:        phygital-key-admin enroll
  2. Connect your AI app: phygital-key-admin token   (prints the line to paste)
DONE
