//! Signed releases and `phygital-key update`.
//!
//! Every release carries `release.txt`, a short list of the release's files and
//! their SHA-256, and `release.txt.sig`, an ECDSA P-256 (SHA-256) signature over
//! it. The release key is a secret of the GitHub environment "release": the
//! release workflow can use it only on a tag, and only after the repository owner
//! approves that run. The public release keys are pinned in
//! `assets/install-service.sh` (one source for the shell installer and this
//! program), so a download mirror, a swapped file or anyone without the owner's
//! approval cannot make an update that installs.
//!
//! `release.txt` is exact, line by line (LF, final LF):
//!
//! ```text
//! phygital-key release manifest v1
//! version v0.4.0
//! <64 lowercase hex>  phygital-key-v0.4.0-aarch64-apple-darwin.tar.gz
//! ...
//! ```

use std::path::{Path, PathBuf};

use p256::ecdsa::signature::Verifier as _;
use p256::ecdsa::{Signature, VerifyingKey};
use sha2::{Digest, Sha256};

/// The secure installer. Its pinned release keys are the ones this program trusts.
pub const INSTALL_SCRIPT: &str = include_str!("../assets/install-service.sh");

const REPO: &str = "Phygital-Key/phygital-key-mcp";
const HEADER: &str = "phygital-key release manifest v1";
const MAX_MANIFEST: usize = 8192;

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

/// The pinned release keys (PEM `PUBLIC KEY` blocks in the installer).
pub fn pinned_keys() -> Vec<VerifyingKey> {
    keys_from_pem(key_block(INSTALL_SCRIPT))
}

/// The text between `# BEGIN RELEASE KEYS` and `# END RELEASE KEYS` (empty if missing).
fn key_block(text: &str) -> &str {
    let (Some(a), Some(b)) = (
        text.find("# BEGIN RELEASE KEYS"),
        text.find("# END RELEASE KEYS"),
    ) else {
        return "";
    };
    text.get(a..b).unwrap_or("")
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
    let sig = Signature::from_der(sig_der).map_err(|_| "release signature is not valid")?;
    // Accept the signature as made (high or low S): the signer is our own openssl.
    let signed = keys
        .iter()
        .any(|k| k.verify(text, &sig).is_ok() || k.verify(text, &sig.normalize_s()).is_ok());
    if keys.is_empty() || !signed {
        return Err("release list is not signed by a pinned release key".into());
    }
    parse_manifest(text)
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
    if lines.next() != Some(HEADER) {
        return bad("unknown format");
    }
    let Some(version) = lines
        .next()
        .and_then(|l| l.strip_prefix("version "))
        .filter(|v| v.starts_with('v'))
        .and_then(Version::parse)
    else {
        return bad("bad version line");
    };
    let prefix = format!("phygital-key-{version}-");
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
    Ok(Manifest { version, files })
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
        "windows" => Err(
            "on Windows, update with Scoop (scoop update phygital-key), or download the new .zip from the Releases page"
                .into(),
        ),
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

/// Fetch and check the latest release list.
fn latest(dir: &Path) -> Result<Manifest, String> {
    let base = format!("https://github.com/{REPO}/releases/latest/download");
    let (m, s) = (dir.join("release.txt"), dir.join("release.txt.sig"));
    download(&format!("{base}/release.txt"), &m)?;
    download(&format!("{base}/release.txt.sig"), &s)?;
    let text = std::fs::read(&m).map_err(|e| e.to_string())?;
    let sig = std::fs::read(&s).map_err(|e| e.to_string())?;
    verify_manifest(&text, &sig, &pinned_keys())
}

/// `phygital-key update [--check]`: replace this program with the latest signed
/// release, if it is newer. For installs in your own folders (the install script,
/// `cargo install`). The secure install and package managers update themselves.
pub fn update(check_only: bool) -> Result<(), String> {
    let exe = std::env::current_exe()
        .and_then(std::fs::canonicalize)
        .map_err(|e| format!("cannot find this program: {e}"))?;
    let shown = exe.display().to_string();
    if shown.starts_with("/Library/PhygitalKey/") || shown.starts_with("/usr/lib/phygital-key/") {
        return Err("this is the secure install: it updates itself every day. To update now: phygital-key-admin update".into());
    }
    if shown.contains("/Cellar/") || shown.contains("/homebrew/") || shown.contains("/linuxbrew/") {
        return Err("installed with Homebrew: run  brew upgrade phygital-key".into());
    }
    let target = target()?;
    let current = Version::parse(env!("CARGO_PKG_VERSION")).ok_or("bad own version")?;
    let dir = private_dir()?;
    let result = (|| {
        let m = latest(&dir)?;
        if m.version <= current {
            println!(
                "phygital-key {current} is up to date (latest signed release: {}).",
                m.version
            );
            return Ok(());
        }
        if check_only {
            println!(
                "phygital-key {} is available (you have {current}). Run: phygital-key update",
                m.version
            );
            return Ok(());
        }
        let name = format!("phygital-key-{}-{target}.tar.gz", m.version);
        let want = m
            .sha256_of(&name)
            .ok_or_else(|| format!("the signed release list has no {name}"))?
            .to_string();
        let tarball = dir.join(&name);
        download(
            &format!(
                "https://github.com/{REPO}/releases/download/{}/{name}",
                m.version
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
        let new = dir
            .join(name.trim_end_matches(".tar.gz"))
            .join("phygital-key");
        let meta = std::fs::symlink_metadata(&new)
            .map_err(|_| "the release does not contain the program")?;
        if !meta.file_type().is_file() {
            return Err("the release does not contain the program".into());
        }
        // Swap in place: write next to this program, then rename over it.
        // A fresh file (never one that was there before, or a link someone placed).
        let staged = exe.with_file_name(".phygital-key.update");
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
        println!("Restart your AI app, or any running phygital-key serve, to use it.");
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// `phygital-key release-check`: used by the secure installer when openssl is
/// missing. Prints the signed version, or fails.
pub fn release_check(file: &Path, sig: &Path) -> Result<(), String> {
    let text = std::fs::read(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let sig = std::fs::read(sig).map_err(|e| format!("{}: {e}", sig.display()))?;
    let m = verify_manifest(&text, &sig, &pinned_keys())?;
    println!("{}", m.version);
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

    #[test]
    fn both_installers_pin_the_same_keys() {
        let user = include_str!("../../../install.sh");
        assert!(!key_block(INSTALL_SCRIPT).is_empty());
        assert_eq!(key_block(INSTALL_SCRIPT), key_block(user));
    }

    #[test]
    fn installer_pins_at_least_one_release_key() {
        assert!(
            !pinned_keys().is_empty(),
            "no release key pinned in assets/install-service.sh: run scripts/release-key.sh"
        );
    }
}
