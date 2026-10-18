//! Append-only, hash-chained audit log (JSON lines).
//!
//! Each line holds `prev` (the hash of the line before) and `hash`
//! (base64url SHA-256 of the line's canonical form without `hash`).
//! Changing, removing or reordering any line breaks the chain.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use phygital_key_verify::{b64, canonical};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

/// The `prev` of the first line.
const GENESIS: &str = "genesis";

/// An open audit log.
pub struct Audit {
    path: PathBuf,
    seq: u64,
    last: String,
}

fn hash_entry(entry: &Map<String, Value>) -> Result<String, String> {
    let bytes = canonical::canonical_bytes(&Value::Object(entry.clone()))
        .map_err(|e| format!("audit: {e:?}"))?;
    Ok(b64::encode(Sha256::digest(bytes)))
}

impl Audit {
    /// Open (or create) the log and check the whole chain.
    pub fn open(path: &Path) -> Result<Audit, String> {
        let (seq, last) = verify_file(path)?;
        Ok(Audit {
            path: path.to_owned(),
            seq,
            last,
        })
    }

    /// Append one event. `fields` must be a JSON object of plain values.
    pub fn append(&mut self, event: &str, fields: Value, ts: i64) -> Result<(), String> {
        let mut entry = match fields {
            Value::Object(m) => m,
            _ => Map::new(),
        };
        entry.insert("seq".into(), Value::from(self.seq + 1));
        entry.insert("ts".into(), Value::from(ts));
        entry.insert("event".into(), Value::from(event));
        entry.insert("prev".into(), Value::from(self.last.clone()));
        let hash = hash_entry(&entry)?;
        entry.insert("hash".into(), Value::from(hash.clone()));
        let line = canonical::canonical_string(&Value::Object(entry))
            .map_err(|e| format!("audit: {e:?}"))?
            + "\n";
        let mut opts = fs::OpenOptions::new();
        opts.append(true).create(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts
            .open(&self.path)
            .map_err(|e| format!("{}: {e}", self.path.display()))?;
        f.write_all(line.as_bytes())
            .map_err(|e| format!("{}: {e}", self.path.display()))?;
        f.sync_data()
            .map_err(|e| format!("{}: {e}", self.path.display()))?;
        self.seq += 1;
        self.last = hash;
        Ok(())
    }
}

/// Check a log file. Returns the number of entries and the last hash.
pub fn verify_file(path: &Path) -> Result<(u64, String), String> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((0, GENESIS.into())),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let (mut seq, mut last) = (0u64, GENESIS.to_string());
    for (n, line) in text.lines().enumerate() {
        let bad = |why: &str| format!("audit log line {}: {why}", n + 1);
        let mut entry = match phygital_key_verify::strict::parse(line.as_bytes()) {
            Some(Value::Object(m)) => m,
            _ => return Err(bad("not a JSON object")),
        };
        let Some(Value::String(hash)) = entry.remove("hash") else {
            return Err(bad("no hash"));
        };
        if entry.get("prev").and_then(Value::as_str) != Some(last.as_str()) {
            return Err(bad("chain broken (prev does not match)"));
        }
        if entry.get("seq").and_then(Value::as_u64) != Some(seq + 1) {
            return Err(bad("sequence broken"));
        }
        if hash_entry(&entry)? != hash {
            return Err(bad("hash does not match (line changed)"));
        }
        seq += 1;
        last = hash;
    }
    Ok((seq, last))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn chain_detects_changes() -> Result<(), String> {
        let dir = std::env::temp_dir().join(format!("pka-audit-{}", crate::util::new_id()?));
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join("audit.log");
        let mut a = Audit::open(&path)?;
        a.append("requested", json!({"id": "a"}), 1)?;
        a.append("approved", json!({"id": "a"}), 2)?;
        assert_eq!(verify_file(&path)?.0, 2);
        let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        fs::write(&path, text.replace("approved", "denied")).map_err(|e| e.to_string())?;
        assert!(verify_file(&path).is_err());
        let first = text.lines().next().unwrap_or_default().to_string() + "\n";
        fs::write(&path, first).map_err(|e| e.to_string())?;
        assert_eq!(
            verify_file(&path)?.0,
            1,
            "truncating the end is not detectable without an anchor"
        );
        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }
}
