#!/bin/sh
# Install phygital-key on macOS or Linux.
#
#   curl -fsSL https://raw.githubusercontent.com/s17sh70x2jfl031ag/phygital-key-mcp/main/install.sh | sh
#
# Downloads the latest release for this machine from GitHub, checks its
# SHA-256, and puts `phygital-key` in ~/.local/bin (or $PHYGITAL_KEY_BIN_DIR).
# Set PHYGITAL_KEY_VERSION=v0.2.0 to pin a version. Nothing else is touched.
set -eu

REPO="s17sh70x2jfl031ag/phygital-key-mcp"
BIN_DIR="${PHYGITAL_KEY_BIN_DIR:-$HOME/.local/bin}"

say() { printf '%s\n' "$*" >&2; }
fail() { say "phygital-key install: $*"; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || fail "needs $1"; }
need curl
need tar
need uname

case "$(uname -s)" in
  Darwin) os=apple-darwin ;;
  Linux) os=unknown-linux-gnu ;;
  *) fail "unsupported system $(uname -s). On Windows use Scoop or the .zip from the Releases page." ;;
esac
case "$(uname -m)" in
  arm64 | aarch64) arch=aarch64 ;;
  x86_64 | amd64) arch=x86_64 ;;
  *) fail "unsupported CPU $(uname -m)" ;;
esac
target="$arch-$os"

version="${PHYGITAL_KEY_VERSION:-}"
if [ -z "$version" ]; then
  version="$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -n 1)"
  [ -n "$version" ] || fail "could not find the latest release"
fi
case "$version" in v[0-9]*) ;; *) fail "bad version $version" ;; esac

name="phygital-key-$version-$target"
url="https://github.com/$REPO/releases/download/$version/$name.tar.gz"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT INT TERM

say "Downloading $name"
curl -fsSL "$url" -o "$tmp/$name.tar.gz" || fail "download failed: $url"
curl -fsSL "$url.sha256" -o "$tmp/$name.tar.gz.sha256" || fail "checksum download failed"

expected="$(cut -d ' ' -f 1 < "$tmp/$name.tar.gz.sha256")"
if command -v sha256sum >/dev/null 2>&1; then
  actual="$(sha256sum "$tmp/$name.tar.gz" | cut -d ' ' -f 1)"
else
  actual="$(shasum -a 256 "$tmp/$name.tar.gz" | cut -d ' ' -f 1)"
fi
[ "$expected" = "$actual" ] || fail "checksum does not match. Nothing was installed."

tar xzf "$tmp/$name.tar.gz" -C "$tmp"
mkdir -p "$BIN_DIR"
install -m 0755 "$tmp/$name/phygital-key" "$BIN_DIR/phygital-key"
say "Installed $BIN_DIR/phygital-key ($version)"
case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *) say "Add it to your PATH: export PATH=\"$BIN_DIR:\$PATH\"" ;;
esac
say "Try it: phygital-key demo"
