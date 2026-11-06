//! The swap record: the one thing that notices a swap which never happened. It is written just
//! before a swap is started, so the next launch can tell whether that version arrived. A version
//! tried [`MAX_ATTEMPTS`] times without arriving is not tried again: something on this machine is
//! stopping it, and the user is told to install it by hand.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::version::is_newer;

pub const MAX_ATTEMPTS: u32 = 2;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwapRecord {
    pub version: String,
    pub attempts: u32,
}

fn file(updates: &Path) -> PathBuf {
    updates.join("swap-record.json")
}

pub fn read(updates: &Path) -> Option<SwapRecord> {
    serde_json::from_slice(&std::fs::read(file(updates)).ok()?).ok()
}

/// A swap of `version` is about to start.
pub fn note_attempt(updates: &Path, version: &str) {
    let attempts = read(updates).filter(|r| r.version == version).map_or(1, |r| r.attempts + 1);
    let record = SwapRecord { version: version.to_owned(), attempts };
    let _ = std::fs::create_dir_all(updates);
    // Losing the record costs at most one more attempt.
    if let Ok(text) = serde_json::to_vec(&record) {
        let _ = std::fs::write(file(updates), text);
    }
}

/// Whether `version` may be swapped in.
pub fn allowed(record: Option<&SwapRecord>, version: &str) -> bool {
    !record.is_some_and(|r| r.version == version && r.attempts >= MAX_ATTEMPTS)
}

/// What a launch learns: the swap it was waiting for happened (the record goes), or it did not
/// (the record is returned).
pub fn settle(updates: &Path, current: &str) -> Option<SwapRecord> {
    let record = read(updates)?;
    if is_newer(&record.version, current) {
        return Some(record);
    }
    let _ = std::fs::remove_file(file(updates));
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_version_that_never_arrives_stops_being_tried() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        assert!(allowed(read(d).as_ref(), "1.0.1"));
        note_attempt(d, "1.0.1");
        assert_eq!(settle(d, "1.0.0"), Some(SwapRecord { version: "1.0.1".into(), attempts: 1 }));
        assert!(allowed(read(d).as_ref(), "1.0.1"), "one more try");
        note_attempt(d, "1.0.1");
        assert!(!allowed(read(d).as_ref(), "1.0.1"));
        assert!(allowed(read(d).as_ref(), "1.0.2"), "a newer version gets its own tries");
        note_attempt(d, "1.0.2");
        assert_eq!(read(d).unwrap().attempts, 1);
    }

    #[test]
    fn a_version_that_arrived_clears_the_record() {
        let dir = tempfile::tempdir().unwrap();
        note_attempt(dir.path(), "1.0.1");
        assert_eq!(settle(dir.path(), "1.0.1"), None);
        assert_eq!(read(dir.path()), None);
    }
}
