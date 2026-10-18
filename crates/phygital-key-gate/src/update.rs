//! Signed releases and `<program> update`.
//!
//! Every release carries `release.txt`, a short list of the release's files and
//! their SHA-256, and `release.txt.sig`, an ECDSA P-256 (SHA-256) signature over
//! it. The release key is a secret of the GitHub environment "release": the
//! release workflow can use it only for `v*` tags, which only repository admins
//! can push. The public release keys come from the product (`Product::release_keys`, the
//! same keys its installers pin), so a download mirror,
//! a swapped file or anyone who cannot push a release tag cannot make an update
//! that installs.
//!
//! `release.txt` is exact, line by line (LF, final LF):
//!
//! ```text
//! phygital-key release manifest v1
//! version v0.4.0
//! <64 lowercase hex>  phygital-key-v0.4.0-aarch64-apple-darwin.tar.gz
//! ...
//! ```
//!
//! Newer releases also carry `release2.txt` (base 0.5.0 on), the same list with the
//! time it was signed and the time it stops being trusted (so a server that keeps
//! handing out an old list is found out), signed twice: `release2.txt.sig`
//! (ECDSA P-256, as above) and `release2.txt.mldsa` (ML-DSA-65, FIPS 204, with the
//! context string `phygital-key release list v2`), a signature that stays safe if
//! large quantum computers arrive. When the product pins an ML-DSA key, both
//! signatures are required.
//!
//! ```text
//! phygital-key release manifest v2
//! version v0.5.0
//! signed 1792300000
//! expires 1807852000
//! <64 lowercase hex>  phygital-key-v0.5.0-aarch64-apple-darwin.tar.gz
//! ...
//! ```

use std::path::{Path, PathBuf};

use p256::ecdsa::signature::Verifier as _;
use p256::ecdsa::{Signature, VerifyingKey};
use sha2::{Digest, Sha256};

const HEADER: &str = "phygital-key release manifest v1";
const HEADER_V2: &str = "phygital-key release manifest v2";
const MAX_MANIFEST: usize = 8192;
/// A v2 list is trusted for at most this long after it was signed.
pub const MAX_LIST_LIFE: i64 = 400 * 86_400;
/// How far ahead of this machine's clock a list may say it was signed.
const SIGNED_SKEW: i64 = 86_400;
/// ML-DSA context string for release lists (FIPS 204 domain separation).
pub const PQ_CONTEXT: &[u8] = b"phygital-key release list v2";
const PQ_BEGIN: &str = "-----BEGIN ML-DSA-65 PUBLIC KEY-----";
const PQ_END: &str = "-----END ML-DSA-65 PUBLIC KEY-----";

/// A pinned ML-DSA-65 release key.
pub type PqKey = ml_dsa::VerifyingKey<ml_dsa::MlDsa65>;

/// A version `vX.Y.Z`, compared by number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(pub u64, pub u64, pub u64);

impl Version {
    /// Parse `vX.Y.Z` (or `X.Y.Z`): plain decimal numbers, no leading zeros.
    pub fn parse(text: &str) -> Option<Version> {
        let t = text.strip_prefix('v').unwrap_or(text);
        let mut parts = t.split('.');
        let mut n = || -> Option<u64> {
            let p = parts.next()?;
            let ok = !p.is_empty()
                && p.len() <= 9
                && p.bytes().all(|b| b.is_ascii_digit())
                && (p == "0" || !p.starts_with('0'));
            if ok { p.parse().ok() } else { None }
        };
        let v = Version(n()?, n()?, n()?);
        if parts.next().is_some() {
            return None;
        }
        Some(v)
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "v{}.{}.{}", self.0, self.1, self.2)
    }
}

/// A checked release manifest.
#[derive(Debug, PartialEq, Eq)]
pub struct Manifest {
    /// The release version the signature covers.
    pub version: Version,
    /// v2 only: when it was signed and when it stops being trusted (Unix seconds).
    pub signed: Option<i64>,
    /// See `signed`.
    pub expires: Option<i64>,
    /// File name and lowercase hex SHA-256, in the signed order.
    pub files: Vec<(String, String)>,
}

impl Manifest {
    /// The SHA-256 the signed manifest gives for `name`.
    pub fn sha256_of(&self, name: &str) -> Option<&str> {
        self.files
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, h)| h.as_str())
    }
}

/// The running product's pinned release keys.
pub fn pinned_keys() -> Vec<VerifyingKey> {
    keys_from_pem(crate::product().release_keys)
}

/// The running product's pinned ML-DSA-65 release keys (none until one is made).
pub fn pinned_pq_keys() -> Vec<PqKey> {
    pq_keys_from_pem(crate::product().release_keys)
}

/// Every `ML-DSA-65 PUBLIC KEY` block in `text` (standard base64 of the 1952-byte
/// encoded key). A block that does not decode to a key is skipped.
pub fn pq_keys_from_pem(text: &str) -> Vec<PqKey> {
    use base64::Engine as _;
    use ml_dsa::KeyInit as _;
    let mut keys = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find(PQ_BEGIN) {
        let after = rest.get(start + PQ_BEGIN.len()..).unwrap_or("");
        let Some(end) = after.find(PQ_END) else {
            break;
        };
        let body: String = after
            .get(..end)
            .unwrap_or("")
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '\'')
            .collect();
        if let Ok(raw) = base64::engine::general_purpose::STANDARD.decode(body)
            && let Ok(k) = PqKey::new_from_slice(&raw)
        {
            keys.push(k);
        }
        rest = after.get(end..).unwrap_or("");
    }
    keys
}

/// The PEM-style block for an ML-DSA-65 public key.
pub fn pq_key_block(key: &PqKey) -> String {
    use base64::Engine as _;
    use ml_dsa::KeyExport as _;
    let b64 = base64::engine::general_purpose::STANDARD.encode(key.to_bytes());
    let mut out = format!("{PQ_BEGIN}\n");
    for chunk in b64.as_bytes().chunks(64) {
        out.push_str(std::str::from_utf8(chunk).unwrap_or(""));
        out.push('\n');
    }
    out.push_str(PQ_END);
    out.push('\n');
    out
}

/// True when `sig` is an ML-DSA-65 signature over `text` by one of `keys`.
pub fn pq_signed(text: &[u8], sig: &[u8], keys: &[PqKey]) -> bool {
    let Ok(sig) = ml_dsa::Signature::<ml_dsa::MlDsa65>::try_from(sig) else {
        return false;
    };
    keys.iter()
        .any(|k| k.verify_with_context(text, PQ_CONTEXT, &sig))
}

/// Check a v2 release list: the P-256 signature, the ML-DSA signature when
/// `pq_keys` is not empty, the strict format, and that it is in date at `now`.
pub fn verify_list_v2(
    text: &[u8],
    sig_der: &[u8],
    pq_sig: Option<&[u8]>,
    keys: &[VerifyingKey],
    pq_keys: &[PqKey],
    now: i64,
) -> Result<Manifest, String> {
    if text.len() > MAX_MANIFEST {
        return Err("release list too large".into());
    }
    p256_signed(text, sig_der, keys)?;
    if !pq_keys.is_empty() && !pq_sig.is_some_and(|s| pq_signed(text, s, pq_keys)) {
        return Err("release list is not signed by the pinned ML-DSA release key".into());
    }
    let m = parse_manifest(text)?;
    let (Some(signed), Some(expires)) = (m.signed, m.expires) else {
        return Err("release list refused: not the v2 format".into());
    };
    if signed > now.saturating_add(SIGNED_SKEW) {
        return Err(
            "release list refused: signed in the future (check this machine's clock)".into(),
        );
    }
    if now >= expires {
        return Err(format!(
            "the latest signed release list expired on {}: no newer one could be found. Check the Releases page",
            crate::util::date(expires)
        ));
    }
    Ok(m)
}

fn p256_signed(text: &[u8], sig_der: &[u8], keys: &[VerifyingKey]) -> Result<(), String> {
    let sig = Signature::from_der(sig_der).map_err(|_| "release signature is not valid")?;
    // Accept the signature as made (high or low S): the signer is our own openssl.
    let signed = keys
        .iter()
        .any(|k| k.verify(text, &sig).is_ok() || k.verify(text, &sig.normalize_s()).is_ok());
    if keys.is_empty() || !signed {
        return Err("release list is not signed by a pinned release key".into());
    }
    Ok(())
}

/// Every P-256 `PUBLIC KEY` PEM block in `text`.
pub fn keys_from_pem(text: &str) -> Vec<VerifyingKey> {
    let mut keys = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("-----BEGIN PUBLIC KEY-----") {
        let after = rest.get(start + 26..).unwrap_or("");
        let Some(end) = after.find("-----END PUBLIC KEY-----") else {
            break;
        };
        let body: String = after
            .get(..end)
            .unwrap_or("")
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '\'')
            .map(|c| match c {
                '+' => '-',
                '/' => '_',
                c => c,
            })
            .collect();
        if let Some(k) = phygital_key_verify::webauthn::parse_public_key(body.trim_end_matches('='))
        {
            keys.push(k);
        }
        rest = after.get(end..).unwrap_or("");
    }
    keys
}

/// Check `release.txt` against its signature and parse it. Refuses anything that
/// is not exactly the format above, or not signed by one of `keys`.
pub fn verify_manifest(
    text: &[u8],
    sig_der: &[u8],
    keys: &[VerifyingKey],
) -> Result<Manifest, String> {
    if text.len() > MAX_MANIFEST {
        return Err("release list too large".into());
    }
    p256_signed(text, sig_der, keys)?;
    let m = parse_manifest(text)?;
    if m.signed.is_some() {
        return Err("release list refused: a v2 list is checked with verify_list_v2".into());
    }
    Ok(m)
}

fn parse_manifest(text: &[u8]) -> Result<Manifest, String> {
    let bad = |why: &str| Err(format!("release list refused: {why}"));
    let Ok(s) = std::str::from_utf8(text) else {
        return bad("not UTF-8");
    };
    let Some(body) = s.strip_suffix('\n') else {
        return bad("must end with a newline");
    };
    let mut lines = body.split('\n');
    let v2 = match lines.next() {
        Some(HEADER) => false,
        Some(HEADER_V2) => true,
        _ => return bad("unknown format"),
    };
    let Some(version) = lines
        .next()
        .and_then(|l| l.strip_prefix("version "))
        .filter(|v| v.starts_with('v'))
        .and_then(Version::parse)
    else {
        return bad("bad version line");
    };
    let (mut signed, mut expires) = (None, None);
    if v2 {
        let mut time = |label: &str| {
            lines
                .next()
                .and_then(|l| l.strip_prefix(label))
                .filter(|n| {
                    !n.is_empty()
                        && n.len() <= 12
                        && n.bytes().all(|b| b.is_ascii_digit())
                        && !n.starts_with('0')
                })
                .and_then(|n| n.parse::<i64>().ok())
        };
        let (Some(s), Some(e)) = (time("signed "), time("expires ")) else {
            return bad("bad signed or expires line");
        };
        if e <= s || e - s > MAX_LIST_LIFE {
            return bad("expires must be after signed, and at most 400 days later");
        }
        (signed, expires) = (Some(s), Some(e));
    }
    let prefix = format!("{}-{version}-", crate::product().bin);
    let mut files: Vec<(String, String)> = Vec::new();
    for line in lines {
        let Some((hash, name)) = line.split_once("  ") else {
            return bad("bad file line");
        };
        let hash_ok =
            hash.len() == 64 && hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
        let name_ok = name.starts_with(&prefix)
            && (name.ends_with(".tar.gz") || name.ends_with(".zip"))
            && name.len() <= 128
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'));
        if !hash_ok || !name_ok || files.iter().any(|(n, _)| n == name) {
            return bad("bad file line");
        }
        files.push((name.to_string(), hash.to_string()));
    }
    if files.is_empty() {
        return bad("no files");
    }
    Ok(Manifest {
        version,
        signed,
        expires,
        files,
    })
}

/// This machine's release target, e.g. `aarch64-apple-darwin`.
fn target() -> Result<String, String> {
    let arch = match std::env::consts::ARCH {
        a @ ("aarch64" | "x86_64") => a,
        a => return Err(format!("no release for this CPU ({a})")),
    };
    match std::env::consts::OS {
        "macos" => Ok(format!("{arch}-apple-darwin")),
        "linux" => Ok(format!("{arch}-unknown-linux-gnu")),
        "windows" => Err(format!(
            "on Windows, update with Scoop (scoop update {}), or download the new .zip from the Releases page",
            crate::product().bin
        )),
        o => Err(format!("no release for this system ({o})")),
    }
}

fn run(cmd: &mut std::process::Command, what: &str) -> Result<(), String> {
    let status = cmd
        .status()
        .map_err(|e| format!("{what}: cannot run: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{what} failed"))
    }
}

fn download(url: &str, to: &Path) -> Result<(), String> {
    run(
        std::process::Command::new("curl")
            // -q first: never read a ~/.curlrc.
            .args([
                "-q",
                "--proto",
                "=https",
                "--tlsv1.2",
                "-fsSL",
                "--max-time",
                "300",
                "-o",
            ])
            .arg(to)
            .arg(url),
        &format!("download of {url}"),
    )
}

fn private_dir() -> Result<PathBuf, String> {
    let mut rnd = [0u8; 12];
    getrandom::fill(&mut rnd).map_err(|e| e.to_string())?;
    let dir = std::env::temp_dir().join(format!(
        "phygital-key-update-{}",
        phygital_key_verify::b64::encode(rnd)
    ));
    #[cfg_attr(not(unix), allow(unused_mut))]
    let mut b = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        b.mode(0o700);
    }
    b.create(&dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?;
    Ok(dir)
}

/// Fetch and check the latest release list (v2: in date, and signed with ML-DSA
/// too when the product pins an ML-DSA key).
fn latest(dir: &Path) -> Result<Manifest, String> {
    let base = format!(
        "https://github.com/{}/releases/latest/download",
        crate::product().repo
    );
    let (m, s, q) = (
        dir.join("release2.txt"),
        dir.join("release2.txt.sig"),
        dir.join("release2.txt.mldsa"),
    );
    download(&format!("{base}/release2.txt"), &m)?;
    download(&format!("{base}/release2.txt.sig"), &s)?;
    let pq_keys = pinned_pq_keys();
    if !pq_keys.is_empty() {
        download(&format!("{base}/release2.txt.mldsa"), &q)?;
    }
    let text = std::fs::read(&m).map_err(|e| e.to_string())?;
    let sig = std::fs::read(&s).map_err(|e| e.to_string())?;
    let pq = std::fs::read(&q).ok();
    verify_list_v2(
        &text,
        &sig,
        pq.as_deref(),
        &pinned_keys(),
        &pq_keys,
        crate::util::now(),
    )
}

/// `phygital-key update [--check]`: replace this program with the latest signed
/// release, if it is newer. For installs in your own folders (the install script,
/// `cargo install`). The secure install and package managers update themselves.
pub fn update(check_only: bool) -> Result<(), String> {
    let exe = std::env::current_exe()
        .and_then(std::fs::canonicalize)
        .map_err(|e| format!("cannot find this program: {e}"))?;
    let shown = exe.display().to_string();
    let p = crate::product();
    let bin = p.bin;
    if shown.starts_with("/Library/PhygitalKey/") || shown.starts_with(&format!("/usr/lib/{bin}/"))
    {
        return Err(format!(
            "this is the secure install: it updates itself every day. To update now: {bin}-admin update"
        ));
    }
    if shown.contains("/Cellar/") || shown.contains("/homebrew/") || shown.contains("/linuxbrew/") {
        return Err(format!("installed with Homebrew: run  brew upgrade {bin}"));
    }
    let target = target()?;
    let current = Version::parse(p.version).ok_or("bad own version")?;
    let dir = private_dir()?;
    let result = (|| {
        let m = latest(&dir)?;
        if m.version <= current {
            println!(
                "{bin} {current} is up to date (latest signed release: {}).",
                m.version
            );
            return Ok(());
        }
        if check_only {
            println!(
                "{bin} {} is available (you have {current}). Run: {bin} update",
                m.version
            );
            return Ok(());
        }
        let name = format!("{bin}-{}-{target}.tar.gz", m.version);
        let want = m
            .sha256_of(&name)
            .ok_or_else(|| format!("the signed release list has no {name}"))?
            .to_string();
        let tarball = dir.join(&name);
        download(
            &format!(
                "https://github.com/{}/releases/download/{}/{name}",
                p.repo, m.version
            ),
            &tarball,
        )?;
        let have = Sha256::digest(std::fs::read(&tarball).map_err(|e| e.to_string())?);
        if hex(&have) != want {
            return Err(
                "the download does not match the signed SHA-256. Nothing was changed.".into(),
            );
        }
        run(
            std::process::Command::new("tar")
                .args(["--no-same-owner", "-xzf"])
                .arg(&tarball)
                .arg("-C")
                .arg(&dir),
            "unpacking",
        )?;
        let new = dir.join(name.trim_end_matches(".tar.gz")).join(bin);
        let meta = std::fs::symlink_metadata(&new)
            .map_err(|_| "the release does not contain the program")?;
        if !meta.file_type().is_file() {
            return Err("the release does not contain the program".into());
        }
        // Swap in place: write next to this program, then rename over it.
        // A fresh file (never one that was there before, or a link someone placed).
        let staged = exe.with_file_name(format!(".{bin}.update"));
        let _ = std::fs::remove_file(&staged);
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o755);
        }
        let mut out = opts
            .open(&staged)
            .map_err(|e| format!("{}: {e}", staged.display()))?;
        let mut src = std::fs::File::open(&new).map_err(|e| e.to_string())?;
        std::io::copy(&mut src, &mut out).map_err(|e| format!("{}: {e}", staged.display()))?;
        out.sync_all().map_err(|e| e.to_string())?;
        drop(out);
        std::fs::rename(&staged, &exe).map_err(|e| {
            let _ = std::fs::remove_file(&staged);
            format!("{}: {e}", exe.display())
        })?;
        println!(
            "Updated {} from {current} to {} (signed release, SHA-256 checked).",
            exe.display(),
            m.version
        );
        println!("Restart your AI app, or any running {bin} serve, to use it.");
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// `<program> release-check`: used by the secure installer. A v1 list prints its
/// version. A v2 list is checked in full (P-256, ML-DSA when a key is pinned, in
/// date now) and prints `<version> <signed> <expires>`. Fails on any doubt.
pub fn release_check(file: &Path, sig: &Path, pq_sig: Option<&Path>) -> Result<(), String> {
    let text = std::fs::read(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let sig = std::fs::read(sig).map_err(|e| format!("{}: {e}", sig.display()))?;
    if text.starts_with(HEADER_V2.as_bytes()) {
        let pq = match pq_sig {
            Some(p) => Some(std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?),
            None => None,
        };
        let m = verify_list_v2(
            &text,
            &sig,
            pq.as_deref(),
            &pinned_keys(),
            &pinned_pq_keys(),
            crate::util::now(),
        )?;
        println!(
            "{} {} {}",
            m.version,
            m.signed.unwrap_or(0),
            m.expires.unwrap_or(0)
        );
    } else {
        let m = verify_manifest(&text, &sig, &pinned_keys())?;
        println!("{}", m.version);
    }
    Ok(())
}

/// `<program> release-sign-pq`: in the release run only. Signs `file` with the
/// ML-DSA-65 key whose 32-byte seed is in `RELEASE_MLDSA_KEY` (standard base64),
/// writes the signature to `out`, and checks it against the pinned keys.
pub fn release_sign_pq(file: &Path, out: &Path) -> Result<(), String> {
    use base64::Engine as _;
    let seed_text = std::env::var("RELEASE_MLDSA_KEY")
        .map_err(|_| "no RELEASE_MLDSA_KEY in the environment")?;
    let seed = base64::engine::general_purpose::STANDARD
        .decode(seed_text.trim())
        .map_err(|_| "RELEASE_MLDSA_KEY is not base64")?;
    let seed: [u8; 32] = seed
        .try_into()
        .map_err(|_| "RELEASE_MLDSA_KEY is not a 32-byte seed")?;
    let key = ml_dsa::SigningKey::<ml_dsa::MlDsa65>::from_seed(&seed.into());
    let text = std::fs::read(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let sig = key
        .expanded_key()
        .sign_deterministic(&text, PQ_CONTEXT)
        .map_err(|_| "ML-DSA signing failed")?;
    let encoded = ml_dsa::signature::SignatureEncoding::to_bytes(&sig);
    let bytes: &[u8] = encoded.as_slice();
    if !pq_signed(&text, bytes, &pinned_pq_keys()) {
        return Err("the ML-DSA signature does not match a key pinned in this program".into());
    }
    std::fs::write(out, bytes).map_err(|e| format!("{}: {e}", out.display()))
}

/// `<program> release-keygen-pq`: once, by the owner (scripts/release-key.sh).
/// Prints a new ML-DSA-65 seed (standard base64) on standard output, for the
/// script to hand straight to GitHub, and writes the public key block to `public_out`.
pub fn release_keygen_pq(public_out: &Path) -> Result<(), String> {
    use base64::Engine as _;
    use ml_dsa::signature::Keypair as _;
    let mut seed = [0u8; 32];
    getrandom::fill(&mut seed).map_err(|e| e.to_string())?;
    let key = ml_dsa::SigningKey::<ml_dsa::MlDsa65>::from_seed(&seed.into());
    std::fs::write(public_out, pq_key_block(&key.verifying_key()))
        .map_err(|e| format!("{}: {e}", public_out.display()))?;
    println!("{}", base64::engine::general_purpose::STANDARD.encode(seed));
    seed.fill(0);
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::*;
    use p256::ecdsa::SigningKey;
    use p256::ecdsa::signature::Signer as _;

    fn key(seed: u8) -> SigningKey {
        SigningKey::from_slice(&[seed; 32]).unwrap_or_else(|_| unreachable!())
    }

    const GOOD: &str = "phygital-key release manifest v1\nversion v0.4.0\n\
        0000000000000000000000000000000000000000000000000000000000000000  phygital-key-v0.4.0-aarch64-apple-darwin.tar.gz\n\
        1111111111111111111111111111111111111111111111111111111111111111  phygital-key-v0.4.0-x86_64-pc-windows-msvc.zip\n";

    fn sign(k: &SigningKey, text: &str) -> Vec<u8> {
        let s: Signature = k.sign(text.as_bytes());
        s.to_der().as_bytes().to_vec()
    }

    #[test]
    fn signed_manifest_is_accepted_and_parsed() {
        let k = key(7);
        let m = verify_manifest(GOOD.as_bytes(), &sign(&k, GOOD), &[*k.verifying_key()]);
        let m = m.unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(m.version, Version(0, 4, 0));
        assert_eq!(m.files.len(), 2);
        assert_eq!(
            m.sha256_of("phygital-key-v0.4.0-x86_64-pc-windows-msvc.zip"),
            Some("1111111111111111111111111111111111111111111111111111111111111111")
        );
    }

    #[test]
    fn refused_when_changed_wrong_key_or_unsigned() {
        let k = key(7);
        let sig = sign(&k, GOOD);
        let changed = GOOD.replace("v0.4.0\n", "v0.5.0\n");
        assert!(verify_manifest(changed.as_bytes(), &sig, &[*k.verifying_key()]).is_err());
        assert!(verify_manifest(GOOD.as_bytes(), &sig, &[*key(8).verifying_key()]).is_err());
        assert!(verify_manifest(GOOD.as_bytes(), &sig, &[]).is_err());
        assert!(verify_manifest(GOOD.as_bytes(), b"junk", &[*k.verifying_key()]).is_err());
    }

    #[test]
    fn strict_format() {
        let k = key(3);
        let vk = [*k.verifying_key()];
        for bad in [
            GOOD.trim_end().to_string(),                        // no final newline
            GOOD.replace("manifest v1", "manifest v2"),         // unknown format
            GOOD.replace("version v0.4.0", "version 0.4.0"),    // no v
            GOOD.replace("version v0.4.0", "version v0.04.0"),  // leading zero
            GOOD.replace("darwin.tar.gz", "darwin.tar.gz.exe"), // odd file
            GOOD.replace("phygital-key-v0.4.0-aarch64", "phygital-key-v0.3.0-aarch64"), // other version
            GOOD.replace("0000000000", "000000000A"), // uppercase hex
            GOOD.replace("  phygital", " phygital"),  // one space
            GOOD.replace("\n", "\r\n"),               // CRLF
            format!("{GOOD}{}", GOOD.lines().nth(2).unwrap_or("")) + "\n", // duplicate file
            "phygital-key release manifest v1\nversion v0.4.0\n".to_string(), // no files
            GOOD.replace("aarch64-apple-darwin", "../../etc/passwd"), // path
        ] {
            assert!(
                verify_manifest(bad.as_bytes(), &sign(&k, &bad), &vk).is_err(),
                "{bad:?}"
            );
        }
    }

    fn pq_key(seed: u8) -> ml_dsa::SigningKey<ml_dsa::MlDsa65> {
        ml_dsa::SigningKey::<ml_dsa::MlDsa65>::from_seed(&[seed; 32].into())
    }

    fn pq_sign(k: &ml_dsa::SigningKey<ml_dsa::MlDsa65>, text: &str) -> Vec<u8> {
        let sig = k
            .expanded_key()
            .sign_deterministic(text.as_bytes(), PQ_CONTEXT)
            .unwrap_or_else(|_| unreachable!());
        ml_dsa::signature::SignatureEncoding::to_bytes(&sig)
            .as_slice()
            .to_vec()
    }

    fn pq_pub(k: &ml_dsa::SigningKey<ml_dsa::MlDsa65>) -> PqKey {
        ml_dsa::signature::Keypair::verifying_key(k)
    }

    const NOW: i64 = 1_792_300_000;

    fn v2(signed: i64, expires: i64) -> String {
        format!(
            "phygital-key release manifest v2\nversion v0.5.0\nsigned {signed}\nexpires {expires}\n\
             0000000000000000000000000000000000000000000000000000000000000000  phygital-key-v0.5.0-aarch64-apple-darwin.tar.gz\n"
        )
    }

    #[test]
    fn v2_list_in_date_and_signed_twice_is_accepted() {
        let (k, q) = (key(7), pq_key(1));
        let text = v2(NOW - 10, NOW + 180 * 86_400);
        let m = verify_list_v2(
            text.as_bytes(),
            &sign(&k, &text),
            Some(&pq_sign(&q, &text)),
            &[*k.verifying_key()],
            &[pq_pub(&q)],
            NOW,
        )
        .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(m.version, Version(0, 5, 0));
        assert_eq!(m.signed, Some(NOW - 10));
        assert_eq!(m.expires, Some(NOW + 180 * 86_400));
        // Without a pinned ML-DSA key, the P-256 signature alone is enough.
        assert!(
            verify_list_v2(
                text.as_bytes(),
                &sign(&k, &text),
                None,
                &[*k.verifying_key()],
                &[],
                NOW
            )
            .is_ok()
        );
    }

    #[test]
    fn v2_list_refused_when_expired_from_the_future_or_missing_a_signature() {
        let (k, q) = (key(7), pq_key(1));
        let vk = [*k.verifying_key()];
        let vq = [pq_pub(&q)];
        let check = |text: &str, pq: Option<Vec<u8>>, pq_keys: &[PqKey], now: i64| {
            verify_list_v2(
                text.as_bytes(),
                &sign(&k, text),
                pq.as_deref(),
                &vk,
                pq_keys,
                now,
            )
        };
        let good = v2(NOW - 10, NOW + 100);
        let pq = Some(pq_sign(&q, &good));
        assert!(check(&good, pq.clone(), &vq, NOW).is_ok());
        let expired = check(&good, pq.clone(), &vq, NOW + 100);
        assert!(expired.is_err_and(|e| e.contains("expired")), "at expires");
        let future = v2(NOW + 86_401, NOW + 200_000);
        assert!(check(&future, Some(pq_sign(&q, &future)), &vq, NOW).is_err());
        assert!(
            check(&good, None, &vq, NOW).is_err(),
            "ML-DSA pinned but missing"
        );
        assert!(
            check(&good, Some(pq_sign(&pq_key(2), &good)), &vq, NOW).is_err(),
            "ML-DSA by another key"
        );
        let other = v2(NOW - 11, NOW + 100);
        assert!(
            check(&good, Some(pq_sign(&q, &other)), &vq, NOW).is_err(),
            "ML-DSA over another list"
        );
        // A v1 list is not a v2 list, and the v1 check does not take a v2 list.
        assert!(check(GOOD, None, &[], NOW).is_err());
        assert!(verify_manifest(good.as_bytes(), &sign(&k, &good), &vk).is_err());
    }

    #[test]
    fn v2_strict_format() {
        let k = key(3);
        let vk = [*k.verifying_key()];
        let good = v2(NOW - 10, NOW + 100);
        for bad in [
            good.replace("signed ", "signed  "),
            good.replace(&format!("signed {}", NOW - 10), "signed 01792299990"),
            good.replace(&format!("signed {}", NOW - 10), "signed -5"),
            good.replace(
                &format!("expires {}", NOW + 100),
                &format!("expires {}", NOW - 10),
            ),
            good.replace(
                &format!("expires {}", NOW + 100),
                &format!("expires {}", NOW - 10 + MAX_LIST_LIFE + 1),
            ),
            good.replace("expires", "expiry"),
            good.replace(&format!("signed {}\n", NOW - 10), ""),
        ] {
            assert!(
                verify_list_v2(bad.as_bytes(), &sign(&k, &bad), None, &vk, &[], NOW).is_err(),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn pq_keys_are_read_from_their_blocks() {
        let q = pq_key(4);
        let block = pq_key_block(&pq_pub(&q));
        let text =
            format!("X='-----BEGIN PUBLIC KEY-----\nAAAA\n-----END PUBLIC KEY-----\n{block}'");
        assert_eq!(pq_keys_from_pem(&text), vec![pq_pub(&q)]);
        assert!(keys_from_pem(&block).is_empty(), "not read as a P-256 key");
        assert!(pq_keys_from_pem("nothing").is_empty());
        assert!(pq_keys_from_pem(&block.replace('A', "B")).len() <= 1);
    }

    /// Google's Wycheproof ML-DSA-65 verify cases, through the same ML-DSA
    /// library and calls as `pq_signed` (with each case's own context).
    #[test]
    fn wycheproof_mldsa_65() {
        use ml_dsa::KeyInit as _;
        let doc: serde_json::Value = serde_json::from_str(include_str!(
            "../../../test-vectors/wycheproof/mldsa_65_verify_test.json"
        ))
        .unwrap_or_default();
        let hex = |s: &str| -> Vec<u8> {
            (0..s.len() / 2)
                .filter_map(|i| s.get(2 * i..2 * i + 2))
                .filter_map(|b| u8::from_str_radix(b, 16).ok())
                .collect()
        };
        let (mut count, mut failures) = (0, Vec::new());
        for g in doc["testGroups"].as_array().into_iter().flatten() {
            let key = PqKey::new_from_slice(&hex(g["publicKey"].as_str().unwrap_or(""))).ok();
            for t in g["tests"].as_array().into_iter().flatten() {
                count += 1;
                let ok = key.as_ref().is_some_and(|k| {
                    ml_dsa::Signature::<ml_dsa::MlDsa65>::try_from(
                        hex(t["sig"].as_str().unwrap_or("")).as_slice(),
                    )
                    .is_ok_and(|sig| {
                        k.verify_with_context(
                            &hex(t["msg"].as_str().unwrap_or("")),
                            &hex(t["ctx"].as_str().unwrap_or("")),
                            &sig,
                        )
                    })
                });
                let want = t["result"].as_str().unwrap_or("");
                if (want == "valid") != ok && want != "acceptable" {
                    failures.push(format!("tcId {} {want}: {}", t["tcId"], t["comment"]));
                }
            }
        }
        assert_eq!(count, doc["numberOfTests"].as_i64().unwrap_or(-1));
        assert!(failures.is_empty(), "{failures:#?}");
    }

    #[test]
    fn versions_compare_by_number() {
        assert!(Version::parse("v0.10.0") > Version::parse("v0.9.9"));
        assert!(Version::parse("v1.0.0") > Version::parse("v0.99.99"));
        assert_eq!(Version::parse("0.3.0"), Some(Version(0, 3, 0)));
        for bad in [
            "v1.2", "v1.2.3.4", "v01.2.3", "v1.2.x", "", "v-1.2.3", "v1..3",
        ] {
            assert_eq!(Version::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn keys_are_read_from_pem_blocks() {
        let k = key(9);
        let pem = p256::pkcs8::EncodePublicKey::to_public_key_der(k.verifying_key())
            .map(|d| {
                use base64::Engine;
                base64::engine::general_purpose::STANDARD.encode(d.as_bytes())
            })
            .unwrap_or_default();
        let text = format!(
            "X='-----BEGIN PUBLIC KEY-----\n{}\n{}\n-----END PUBLIC KEY-----\n'",
            &pem[..64],
            &pem[64..]
        );
        assert_eq!(keys_from_pem(&text), vec![*k.verifying_key()]);
        assert!(keys_from_pem("nothing").is_empty());
    }
}
