//! A seen store in a file, shared by every process on one machine.
//!
//! Each line is `{"id": ..., "exp": ...}`. A separate `.lock` file is held
//! while the list is read and rewritten; the new list is written to a temp
//! file, flushed, then renamed over the old one, so a crash leaves either the
//! old list or the new one, never half of one. Any line that cannot be read
//! makes every check fail (closed), instead of quietly forgetting ids.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use phygital_key_verify::SeenStore;
use serde_json::{Value, json};

/// Used approval ids, kept in a file until they expire.
pub struct FileSeenStore {
    path: PathBuf,
}

fn private_open(path: &PathBuf, truncate: bool) -> Result<File, String> {
    let mut opts = OpenOptions::new();
    opts.read(true).write(true).create(true).truncate(truncate);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    opts.open(path)
        .map_err(|e| format!("{}: {e}", path.display()))
}

impl FileSeenStore {
    /// A store at `path` (created on first use).
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn insert(&self, id: &str, exp: i64, now: i64) -> Result<bool, String> {
        let lock = private_open(&self.path.with_extension("lock"), false)?;
        lock.lock().map_err(|e| format!("lock: {e}"))?;
        let text = match std::fs::read_to_string(&self.path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e.to_string()),
        };
        let mut keep: Vec<(String, i64)> = Vec::new();
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            let v: Value =
                serde_json::from_str(line).map_err(|_| "seen file is damaged".to_string())?;
            let (Some(i), Some(e)) = (
                v.get("id").and_then(Value::as_str),
                v.get("exp").and_then(Value::as_i64),
            ) else {
                return Err("seen file is damaged".into());
            };
            if e > now {
                keep.push((i.to_string(), e));
            }
        }
        if keep.iter().any(|(i, _)| i == id) {
            return Ok(false);
        }
        keep.push((id.to_string(), exp));
        let mut out = String::new();
        for (i, e) in &keep {
            out.push_str(&json!({"id": i, "exp": e}).to_string());
            out.push('\n');
        }
        let tmp = self.path.with_extension("tmp");
        let mut f = private_open(&tmp, true)?;
        f.write_all(out.as_bytes()).map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &self.path).map_err(|e| e.to_string())?;
        // The lock is released when `lock` is dropped.
        Ok(true)
    }
}

impl SeenStore for FileSeenStore {
    fn insert_if_new(&self, id: &str, exp: i64, now: i64) -> bool {
        // Any error means "not new": fail closed.
        self.insert(id, exp, now).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remembers_until_expiry_and_fails_closed() -> Result<(), String> {
        let dir = std::env::temp_dir().join(format!("pka-seen-{}", crate::util::new_id()?));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join("seen.jsonl");
        let s = FileSeenStore::new(path.clone());
        assert!(s.insert_if_new("a", 100, 10));
        assert!(!s.insert_if_new("a", 100, 20), "second use refused");
        assert!(FileSeenStore::new(path.clone()).insert_if_new("b", 100, 20));
        assert!(
            !FileSeenStore::new(path.clone()).insert_if_new("a", 100, 30),
            "survives a new process"
        );
        assert!(
            s.insert_if_new("a", 300, 100),
            "forgotten after expiry (it can no longer verify anyway)"
        );
        // A damaged file refuses everything instead of forgetting.
        std::fs::write(&path, "{\"id\":\"x\",\"exp\":999}\nnot json\n")
            .map_err(|e| e.to_string())?;
        assert!(
            !s.insert_if_new("new", 1000, 10),
            "damaged file fails closed"
        );
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }
}
