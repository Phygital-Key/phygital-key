#!/bin/sh
# shellcheck disable=SC2034 # every script carries the whole PRODUCT block; not all use every value
# Install a Phygital Key product on macOS or Linux (the PRODUCT block below says
# which). Generated from the template in Phygital-Key/phygital-key.
#
#   curl -fsSL https://github.com/<product repo>/releases/latest/download/install.sh | sh
#
# Downloads the latest release for this machine from GitHub, checks that its
# release list is signed by a pinned release key and that the SHA-256 matches, and puts the program in ~/.local/bin (or $PHYGITAL_KEY_BIN_DIR).
# Set PHYGITAL_KEY_VERSION=v0.3.0 (or later: earlier releases have no signed list) to pin a version. Nothing else is touched.
set -eu

# BEGIN PRODUCT
P_NAME="Phygital Key"
P_BIN=phygital-key
P_ID=dev
P_REPO=Phygital-Key/phygital-key
P_PORT=7468
P_VERSION=v0.0.0
# END PRODUCT
REPO="$P_REPO"
BIN_DIR="${PHYGITAL_KEY_BIN_DIR:-$HOME/.local/bin}"

say() { printf '%s\n' "$*" >&2; }
fail() { say "$P_BIN install: $*"; exit 1; }
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

# Pinned release keys (P-256), the same block as in the secure installer.
# BEGIN RELEASE KEYS
RELEASE_KEYS='
'
# END RELEASE KEYS

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT INT TERM

version="${PHYGITAL_KEY_VERSION:-}"
if [ -n "$version" ]; then
  case "$version" in v[0-9]*.[0-9]*.[0-9]*) ;; *) fail "bad version $version" ;; esac
  base="https://github.com/$REPO/releases/download/$version"
else
  base="https://github.com/$REPO/releases/latest/download"
fi

# The release list (file names and SHA-256) must be signed by a pinned release key.
curl -fsSL "$base/release.txt" -o "$tmp/release.txt" || fail "could not download the signed release list"
curl -fsSL "$base/release.txt.sig" -o "$tmp/release.txt.sig" || fail "could not download the release signature"
need openssl
printf '%s\n' "$RELEASE_KEYS" | awk -v dir="$tmp" '
  /-----BEGIN PUBLIC KEY-----/ { n++; f = dir "/key-" n ".pem" }
  f { print > f }
  /-----END PUBLIC KEY-----/ { close(f); f = "" }'
signed=no
for k in "$tmp"/key-*.pem; do
  [ -f "$k" ] || continue
  openssl dgst -sha256 -verify "$k" -signature "$tmp/release.txt.sig" "$tmp/release.txt" >/dev/null 2>&1 && signed=yes
done
[ "$signed" = yes ] || fail "the release list is not signed by a pinned release key. Nothing was installed."
[ "$(sed -n 1p "$tmp/release.txt")" = "phygital-key release manifest v1" ] || fail "unknown release list format"
got="$(sed -n 2p "$tmp/release.txt" | sed -n 's/^version \(v[0-9][0-9]*\.[0-9][0-9]*\.[0-9][0-9]*\)$/\1/p')"
[ -n "$got" ] || fail "bad release list"
[ -z "$version" ] || [ "$got" = "$version" ] || fail "the signed release list is for $got, not $version"
version="$got"

name="$P_BIN-$version-$target"
expected="$(awk -v n="$name.tar.gz" '$2 == n && length($1) == 64 { print $1 }' "$tmp/release.txt")"
[ -n "$expected" ] || fail "the signed release list has no $name.tar.gz"

say "Downloading $name (signed release list checked)"
curl -fsSL "https://github.com/$REPO/releases/download/$version/$name.tar.gz" -o "$tmp/$name.tar.gz" || fail "download failed"
if command -v sha256sum >/dev/null 2>&1; then
  actual="$(sha256sum "$tmp/$name.tar.gz" | cut -d ' ' -f 1)"
else
  actual="$(shasum -a 256 "$tmp/$name.tar.gz" | cut -d ' ' -f 1)"
fi
[ "$expected" = "$actual" ] || fail "the download does not match its signed SHA-256. Nothing was installed."

tar --no-same-owner -xzf "$tmp/$name.tar.gz" -C "$tmp"
mkdir -p "$BIN_DIR"
install -m 0755 "$tmp/$name/$P_BIN" "$BIN_DIR/$P_BIN"
say "Installed $BIN_DIR/$P_BIN ($version)"
case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *) say "Add it to your PATH: export PATH=\"$BIN_DIR:\$PATH\"" ;;
esac
say "Try it: $P_BIN demo"
say "Update later with: $P_BIN update"
