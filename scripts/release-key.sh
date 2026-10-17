#!/bin/bash
# Make the Phygital Key release keys (run once, on the maintainer's own Mac).
#
#   scripts/release-key.sh
#
# Creates two P-256 keys in ~/.phygital-key-release (only you can open it):
#   signing-key.pem  signs every release (scripts/sign-release.sh)
#   backup-key.pem   kept off this Mac, used only if the signing key is lost
# Each key file is encrypted (AES-256). Its passphrase is a random value stored in
# your login Keychain ("phygital-key-release-signing" / "-backup"), so the files
# alone are useless to anyone who copies them.
# Then it pins both public keys in the two installers of this repo. Commit that
# change: from then on only releases signed with one of these keys install or update.
set -euo pipefail
cd "$(dirname "$0")/.."
D="$HOME/.phygital-key-release"
command -v openssl >/dev/null || { echo "needs openssl"; exit 1; }
command -v security >/dev/null || { echo "needs macOS (security, for the Keychain)"; exit 1; }
[ ! -e "$D/signing-key.pem" ] || { echo "$D/signing-key.pem already exists. Not touching it."; exit 1; }
umask 077
mkdir -p "$D"; chmod 700 "$D"
for k in signing backup; do
  pass=$(openssl rand -base64 33 | tr -d '\n')
  security add-generic-password -U -a phygital-key-release -s "phygital-key-release-$k" \
    -l "Phygital Key release $k key" -w "$pass"
  P="$pass" openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 \
    -aes-256-cbc -pass env:P -out "$D/$k-key.pem"
  P="$pass" openssl pkey -in "$D/$k-key.pem" -passin env:P -pubout -out "$D/$k-pub.pem"
  unset pass
done
block=$(cat "$D/signing-pub.pem" "$D/backup-pub.pem")
for f in crates/phygital-key/assets/install-service.sh install.sh; do
  BLOCK="$block" python3 - "$f" <<'PY'
import os, re, sys
p = sys.argv[1]; s = open(p).read()
new = "# BEGIN RELEASE KEYS\nRELEASE_KEYS='\n" + os.environ["BLOCK"].strip() + "\n'\n# END RELEASE KEYS"
s2, n = re.subn(r"# BEGIN RELEASE KEYS\n.*?# END RELEASE KEYS", lambda m: new, s, flags=re.S)
assert n == 1, p
open(p, "w").write(s2)
PY
done
cat <<DONE
Done. Keys in $D:
  signing-key.pem   stays here; scripts/sign-release.sh uses it
  backup-key.pem    MOVE IT OFF THIS MAC now: copy it, and its passphrase from
                    Keychain Access (item "phygital-key-release-backup"), into your
                    password manager or a USB stick, then delete $D/backup-key.pem
Both public keys are now pinned in:
  crates/phygital-key/assets/install-service.sh
  install.sh
Commit them before the next release.
DONE
