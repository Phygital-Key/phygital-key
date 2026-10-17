//! A seen store in a file, shared by every process on one machine.
//! Each line is `{"id": ..., "exp": ...}`. The file is locked while checked.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

use phygital_key_verify::SeenStore;
use serde_json::{Value, json};

/// Used approval ids, kept in a file until they expire.
pub struct FileSeenStore {
    path: PathBuf,
}

impl FileSeenStore {
    /// A store at `path` (created on first use).
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn insert(&self, id: &str, exp: i64, now: i64) -> Result<bool, String> {
        let mut opts = OpenOptions::new();
        opts.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f: File = opts
            .open(&self.path)
            .map_err(|e| format!("{}: {e}", self.path.display()))?;
        f.lock()
            .map_err(|e| format!("lock {}: {e}", self.path.display()))?;
        let mut text = String::new();
        f.read_to_string(&mut text).map_err(|e| e.to_string())?;
        let mut keep: Vec<(String, i64)> = Vec::new();
        for line in text.lines() {
            let Ok(v) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            let (Some(i), Some(e)) = (
                v.get("id").and_then(Value::as_str),
                v.get("exp").and_then(Value::as_i64),
            ) else {
                continue;
            };
            if e > now {
                keep.push((i.to_string(), e));
            }
        }
        let fresh = !keep.iter().any(|(i, _)| i == id);
        if fresh {
            keep.push((id.to_string(), exp));
        }
        let mut out = String::new();
        for (i, e) in &keep {
            out.push_str(&json!({"id": i, "exp": e}).to_string());
            out.push('\n');
        }
        f.set_len(0).map_err(|e| e.to_string())?;
        f.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
        f.write_all(out.as_bytes()).map_err(|e| e.to_string())?;
        f.sync_data().map_err(|e| e.to_string())?;
        // Unlocks when `f` is dropped.
        Ok(fresh)
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
    fn remembers_until_expiry() -> Result<(), String> {
        let dir = std::env::temp_dir().join(format!("pka-seen-{}", crate::util::new_id()?));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let s = FileSeenStore::new(dir.join("seen.jsonl"));
        assert!(s.insert_if_new("a", 100, 10));
        assert!(!s.insert_if_new("a", 100, 20), "second use refused");
        assert!(FileSeenStore::new(dir.join("seen.jsonl")).insert_if_new("b", 100, 20));
        assert!(
            !FileSeenStore::new(dir.join("seen.jsonl")).insert_if_new("a", 100, 30),
            "survives a new process"
        );
        assert!(
            s.insert_if_new("a", 300, 100),
            "forgotten after expiry (it can no longer verify anyway)"
        );
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }
}
