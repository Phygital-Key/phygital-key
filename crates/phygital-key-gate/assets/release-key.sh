#!/bin/bash
# shellcheck disable=SC2034 # every script carries the whole PRODUCT block; not all use every value
# Set up release signing (run once, by the repository owner, in a terminal).
#
#   scripts/release-key.sh
#
# What it does:
#   1. Makes a P-256 release signing key in a temporary folder.
#   2. Creates the GitHub environment "release", which only tags v* can use.
#      (To also wait for your approval on every run: Settings, Environments,
#      release, Required reviewers.)
#   3. Stores the private key as the environment secret RELEASE_SIGNING_KEY. Only a
#      release run for a v* tag can read it. Then it deletes the local copy.
#   4. Pins the public key in release-keys.pem and the installers (commit that change).
# From then on, only releases signed by that key install or update.
#
# Protect your GitHub account well (two-factor sign-in, ideally a passkey): whoever
# controls it can push a release tag.
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
if grep -q -- "-----BEGIN PUBLIC KEY-----" release-keys.pem 2>/dev/null; then
  echo "A release key is already pinned in release-keys.pem. Not making another one."; exit 1
fi

T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
umask 077
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
cat <<DONE
Done.
  - The private key is only in GitHub now (environment "release", secret RELEASE_SIGNING_KEY).
    The local copy is deleted.
  - The public key is in release-keys.pem, install-service.sh and install.sh. Commit them.
  - Each release: push a tag vX.Y.Z. CI builds, signs and publishes it.
DONE
