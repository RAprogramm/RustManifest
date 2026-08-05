// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Keeping the store bounded.
//!
//! Entries are addressed by content, so nothing ever invalidates one: every
//! edit of every file leaves a new entry behind and the directory grows for
//! the life of the checkout. Pruning happens when the cache is opened rather
//! than on every store, so the cost is one directory read per run instead of
//! one per file, and the least recently modified entries go first.

use std::{fs, path::Path, time::SystemTime};

use tracing::warn;

/// Drops the least recently modified entries of `dir` until at most
/// `capacity` remain.
///
/// Every failure is logged at `warn` level and otherwise ignored: a cache
/// that cannot be pruned is still a correct cache, only a larger one.
pub(super) fn to_capacity(dir: &Path, capacity: usize) {
    let mut entries = match dated_entries(dir) {
        Ok(entries) => entries,
        Err(err) => {
            warn!(dir = %dir.display(), error = %err, "cache directory unreadable; not pruned");
            return;
        }
    };
    if entries.len() <= capacity {
        return;
    }
    entries.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
    let excess = entries.len() - capacity;
    for (_, path) in entries.into_iter().take(excess) {
        if let Err(err) = fs::remove_file(&path) {
            warn!(entry = %path.display(), error = %err, "cache entry not removed");
        }
    }
}

/// Lists the entry files of `dir` paired with their modification time,
/// oldest-first ordering left to the caller.
///
/// A file whose modification time is unavailable is treated as the oldest
/// possible, so an unreadable timestamp makes an entry a pruning candidate
/// rather than an immortal one.
fn dated_entries(dir: &Path) -> std::io::Result<Vec<(SystemTime, std::path::PathBuf)>> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().is_none_or(|extension| extension != "json") {
            continue;
        }
        let modified = fs::metadata(&path)
            .and_then(|metadata| metadata.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        entries.push((modified, path));
    }
    Ok(entries)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use std::{thread::sleep, time::Duration};

    use tempfile::tempdir;

    use super::*;

    fn write(dir: &Path, name: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        fs::write(&path, b"[]").unwrap();
        path
    }

    fn json_count(dir: &Path) -> usize {
        fs::read_dir(dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "json")
            })
            .count()
    }

    #[test]
    fn a_store_under_capacity_is_left_alone() {
        let dir = tempdir().unwrap();
        write(dir.path(), "a.json");
        write(dir.path(), "b.json");
        to_capacity(dir.path(), 4);
        assert_eq!(json_count(dir.path()), 2);
    }

    #[test]
    fn the_oldest_entries_go_first() {
        let dir = tempdir().unwrap();
        let oldest = write(dir.path(), "oldest.json");
        sleep(Duration::from_millis(20));
        let newest = write(dir.path(), "newest.json");
        to_capacity(dir.path(), 1);
        assert!(!oldest.exists());
        assert!(newest.exists());
    }

    #[test]
    fn files_that_are_not_entries_are_never_touched() {
        let dir = tempdir().unwrap();
        let foreign = dir.path().join("notes.txt");
        fs::write(&foreign, b"keep me").unwrap();
        write(dir.path(), "a.json");
        write(dir.path(), "b.json");
        to_capacity(dir.path(), 0);
        assert_eq!(json_count(dir.path()), 0);
        assert!(foreign.exists());
    }

    #[test]
    fn a_missing_directory_is_not_an_error() {
        to_capacity(Path::new("does/not/exist"), 1);
    }
}
