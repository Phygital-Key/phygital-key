#!/bin/bash
# Sign a release and publish it (run on the maintainer's Mac, after CI built it).
#
#   scripts/sign-release.sh v0.3.0
#
# The release workflow makes a DRAFT release. This script downloads its files,
# computes their SHA-256 itself, writes release.txt, signs it with the release key
# from ~/.phygital-key-release (you type its passphrase), checks the signature
# against the keys pinned in this repo's installer, uploads release.txt and
# release.txt.sig, and only then publishes the release. Until then no installer and
# no automatic update can see it.
set -euo pipefail
cd "$(dirname "$0")/.."
V="${1:-}"
[ -t 0 ] && [ -t 1 ] || { echo "Run this yourself, in a terminal: it asks for the key passphrase."; exit 1; }
[[ "$V" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "usage: $0 vX.Y.Z"; exit 2; }
D="$HOME/.phygital-key-release"
[ -f "$D/signing-key.pem" ] || { echo "no release key: run scripts/release-key.sh first"; exit 1; }
REPO=s17sh70x2jfl031ag/phygital-key-mcp
# The commit YOUR clone has for this tag: the files must have been built from it.
COMMIT=$(git rev-parse --verify "refs/tags/$V^{commit}") || { echo "no tag $V in this clone"; exit 1; }
echo "Tag $V is commit $COMMIT in this clone."
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
gh release download "$V" --repo "$REPO" --dir "$T" --pattern "phygital-key-$V-*"
cd "$T"
{
  echo "phygital-key release manifest v1"
  echo "version $V"
  for f in $(ls phygital-key-"$V"-*.tar.gz phygital-key-"$V"-*.zip 2>/dev/null | sort); do
    h=$(shasum -a 256 "$f" | cut -d ' ' -f 1)
    # The workflow's own .sha256 must agree (catches a broken upload).
    [ "$(cut -d ' ' -f 1 < "$f.sha256")" = "$h" ] || { echo "SHA-256 of $f does not match its .sha256" >&2; exit 1; }
    # Built by this repo's release workflow, from this tag, at the commit above.
    # (A file swapped on GitHub after the build has no such record.)
    gh attestation verify "$f" --repo "$REPO" \
      --signer-workflow "$REPO/.github/workflows/release.yml" \
      --source-ref "refs/tags/$V" --source-digest "$COMMIT" >/dev/null ||
      { echo "$f: no build record from the release workflow at $COMMIT. Not signing." >&2; exit 1; }
    echo "$h  $f"
  done
} > release.txt
[ "$(wc -l < release.txt)" -ge 3 ] || { echo "no release files found" >&2; exit 1; }
echo "Type the signing key passphrase:"
openssl dgst -sha256 -sign "$D/signing-key.pem" -out release.txt.sig release.txt
# Check against the keys this repo pins (what installers will use).
ok=no
awk '/-----BEGIN PUBLIC KEY-----/{n++; f="pin-" n ".pem"} f{print > f} /-----END PUBLIC KEY-----/{close(f); f=""}' \
  "$OLDPWD/crates/phygital-key/assets/install-service.sh"
for k in pin-*.pem; do
  [ -f "$k" ] && openssl dgst -sha256 -verify "$k" -signature release.txt.sig release.txt >/dev/null 2>&1 && ok=yes
done
[ "$ok" = yes ] || { echo "the signature does not match any key pinned in install-service.sh. Not publishing." >&2; exit 1; }
cat release.txt
gh release upload "$V" --repo "$REPO" --clobber release.txt release.txt.sig
gh release edit "$V" --repo "$REPO" --draft=false --latest
echo "Signed and published $V."
