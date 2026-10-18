//! Small helpers: time, randomness, dates, files.

use std::fs;
use std::io::Write;
use std::path::Path;

use phygital_key_verify::b64;

/// Unix seconds now.
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

/// `n` random bytes from the operating system.
pub fn random<const N: usize>() -> Result<[u8; N], String> {
    let mut buf = [0u8; N];
    getrandom::fill(&mut buf).map_err(|e| format!("random: {e}"))?;
    Ok(buf)
}

/// A random 128-bit id, base64url (22 characters).
pub fn new_id() -> Result<String, String> {
    Ok(b64::encode(random::<16>()?))
}

/// `YYYY-MM-DD` for a Unix time (UTC).
pub fn date(unix: i64) -> String {
    // Howard Hinnant's days-to-civil algorithm.
    let z = unix.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Write a file atomically (temp file then rename), private to this user.
pub fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    {
        let mut opts = fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts
            .open(&tmp)
            .map_err(|e| format!("{}: {e}", tmp.display()))?;
        f.write_all(bytes)
            .map_err(|e| format!("{}: {e}", tmp.display()))?;
        f.sync_all()
            .map_err(|e| format!("{}: {e}", tmp.display()))?;
    }
    fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Create a directory private to this user.
pub fn private_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|e| format!("{}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn dates() {
        assert_eq!(super::date(0), "1970-01-01");
        assert_eq!(super::date(1_760_700_000), "2025-10-17");
        assert_eq!(super::date(951_782_400), "2000-02-29");
    }
}
