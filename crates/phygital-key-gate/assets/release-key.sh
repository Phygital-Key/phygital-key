#!/bin/bash
# shellcheck disable=SC2034 # every script carries the whole PRODUCT block; not all use every value
# Set up release signing (run once, by the repository owner, in a terminal).
#
#   scripts/release-key.sh          the P-256 key, then the ML-DSA key
#   scripts/release-key.sh --pq     only the ML-DSA key (the P-256 key is pinned already)
#
# What it does:
#   1. Makes a P-256 release signing key in a temporary folder.
#   2. Creates the GitHub environment "release", which only tags v* can use.
#      (To also wait for your approval on every run: Settings, Environments,
#      release, Required reviewers.)
#   3. Stores the private key as the environment secret RELEASE_SIGNING_KEY. Only a
#      release run for a v* tag can read it. Then it deletes the local copy.
#   4. Pins the public key in release-keys.pem and the installers (commit that change).
#   5. The same for a second, post-quantum key (ML-DSA-65, FIPS 204): the program
#      makes it, the private part goes straight to the secret RELEASE_MLDSA_KEY
#      without being written to disk, and the public part is pinned next to the first.
# From then on, only releases signed by those keys install or update.
set -euo pipefail
cd "$(dirname "$0")/.."
# Generated for one product from the template in Phygital-Key/phygital-key.
# BEGIN PRODUCT
P_NAME="Phygital Key"
P_BIN=phygital-key
P_ID=dev
P_REPO=Phygital-Key/phygital-key
P_PORT=7468
P_VERSION=v0.0.0
# END PRODUCT
REPO="$P_REPO"
[ -t 0 ] && [ -t 1 ] || { echo "Run this yourself, in a terminal."; exit 1; }
command -v openssl >/dev/null || { echo "needs openssl"; exit 1; }
command -v gh >/dev/null || { echo "needs the GitHub CLI (gh), signed in as the repository owner"; exit 1; }
gh auth status >/dev/null 2>&1 || { echo "sign in first: gh auth login"; exit 1; }
PQ_ONLY=no
[ "${1:-}" = --pq ] && PQ_ONLY=yes
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
umask 077

make_pq_key() {
  if grep -q -- "-----BEGIN ML-DSA-65 PUBLIC KEY-----" release-keys.pem 2>/dev/null; then
    echo "An ML-DSA release key is already pinned. Not making another one."; return 0
  fi
  echo "Building the program and making the ML-DSA key"
  # cargo run finds the program wherever cargo builds it. The seed stays in this
  # shell's memory only, and is checked before anything is stored.
  seed=$(cargo run -q --release --locked -- release-keygen-pq --public-out "$T/pq.pem") || seed=""
  if ! [[ "$seed" =~ ^[A-Za-z0-9+/]{43}=$ ]] || ! grep -q "BEGIN ML-DSA-65 PUBLIC KEY" "$T/pq.pem" 2>/dev/null; then
    seed=""; echo "Making the ML-DSA key failed. Nothing was stored or pinned."; exit 1
  fi
  echo "Storing the ML-DSA key as the secret RELEASE_MLDSA_KEY of the environment \"release\""
  printf '%s' "$seed" | gh secret set RELEASE_MLDSA_KEY --env release --repo "$REPO"
  seed=""
  cat "$T/pq.pem" >> release-keys.pem
  echo "Pinned the ML-DSA public key in release-keys.pem; writing the installers again"
  cargo run -q --release -- product-files --out .
}

if [ "$PQ_ONLY" = yes ]; then
  grep -q -- "-----BEGIN PUBLIC KEY-----" release-keys.pem 2>/dev/null ||
    { echo "No P-256 release key is pinned yet: run this without --pq first."; exit 1; }
  make_pq_key
  cat <<DONE
Done.
  - The ML-DSA private key is only in GitHub (environment "release", secret RELEASE_MLDSA_KEY).
  - Its public key is in release-keys.pem, install-service.sh and install.sh. Commit them.
  - From the next release on, every release list is signed with both keys.
DONE
  exit 0
fi

if grep -q -- "-----BEGIN PUBLIC KEY-----" release-keys.pem 2>/dev/null; then
  echo "A release key is already pinned in release-keys.pem. Not making another one."
  echo "To add the post-quantum key: scripts/release-key.sh --pq"; exit 1
fi

openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out "$T/key.pem" 2>/dev/null
openssl pkey -in "$T/key.pem" -pubout -out "$T/pub.pem"

echo "Creating the GitHub environment \"release\" (tags v* only)"
gh api -X PUT "repos/$REPO/environments/release" --input - >/dev/null <<JSON
{"reviewers": [],
 "deployment_branch_policy": {"protected_branches": false, "custom_branch_policies": true}}
JSON
gh api -X POST "repos/$REPO/environments/release/deployment-branch-policies" \
  -f name='v*' -f type=tag >/dev/null 2>&1 || true   # already there on a second run

echo "Storing the private key as the secret RELEASE_SIGNING_KEY of that environment"
gh secret set RELEASE_SIGNING_KEY --env release --repo "$REPO" < "$T/key.pem"

cp "$T/pub.pem" release-keys.pem
echo "Pinned the public key in release-keys.pem; writing the installers again"
cargo run -q --release -- product-files --out .
make_pq_key
cat <<DONE
Done.
  - The private keys are only in GitHub now (environment "release", secrets
    RELEASE_SIGNING_KEY and RELEASE_MLDSA_KEY). The local copy is deleted.
  - The public key is in release-keys.pem, install-service.sh and install.sh. Commit them.
  - Each release: push a tag vX.Y.Z. CI builds, signs and publishes it.
DONE
