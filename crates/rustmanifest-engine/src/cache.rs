// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Optional persistent findings cache, content-addressed per file.
//!
//! A [`FindingsCache`] is opened at a directory and stores one JSON file per
//! cache entry, named `<key>.json`, where the key is the hex-encoded FNV-1a
//! 128-bit hash of the concatenation of:
//!
//! - the raw bytes of the analyzed file,
//! - the sorted rule-id set plus every rule definition serialized as JSON (see
//!   [`rules_fingerprint`]),
//! - the engine crate version (`CARGO_PKG_VERSION`).
//!
//! An entry holds the serialized findings produced by a full orchestrator
//! pass over that file, **including** the engine-synthesized `RM-META-001`
//! unused-pragma findings, so a cache hit reproduces the fresh run
//! byte-for-byte. It also records the byte lengths it was built from, and a
//! lookup whose lengths disagree is refused: an address collision then costs
//! a re-analysis instead of silently answering about the wrong file.
//!
//! Because the key is derived from file content rather than the file path,
//! [`FindingsCache::lookup`] rewrites each cached finding's location to the
//! path being analyzed; identical files therefore share one entry without
//! leaking each other's paths.
//!
//! Nothing invalidates a content address, so the store is bounded instead:
//! opening a cache drops its least recently modified entries down to the
//! configured capacity.
//!
//! The workspace-level **semantic tier runs outside the orchestrator** (the
//! CLI drives it over the whole file set) and is deliberately **not** covered
//! by this cache: its findings depend on cross-file state that a per-file
//! content address cannot capture.
//!
//! Cache failures are never fatal: unreadable or corrupt entries are treated
//! as misses (and overwritten on the next store), and store failures are
//! logged at `warn` level while the run continues with fresh results.

mod entry;
mod key;
mod prune;

use std::{
    fs, io,
    path::{Path, PathBuf}
};

use rustmanifest_schema::Finding;
use tracing::{debug, warn};

use self::entry::Entry;
pub use self::key::rules_fingerprint;

/// Computes the cache key for one file: the hex-encoded FNV-1a 128-bit hash
/// of `file_bytes` followed by `rules_fingerprint` (as produced by
/// [`rules_fingerprint`]).
///
/// A free function rather than a method: the address is a property of the
/// bytes, and computing it needs no store to exist.
#[must_use]
pub fn key(file_bytes: &[u8], rules_fingerprint: &[u8]) -> String {
    key::of(file_bytes, rules_fingerprint)
}

/// Persistent, content-addressed store for per-file orchestrator findings.
///
/// See the [module documentation](self) for the key composition and the
/// failure model. Cloning is cheap; clones share the same directory.
#[derive(Debug, Clone)]
pub struct FindingsCache {
    /// Directory holding one JSON file per entry.
    dir:   PathBuf,
    /// Length of the rule-set fingerprint entries here were built under.
    rules: usize
}

impl FindingsCache {
    /// Entries kept by default before the least recently modified are
    /// dropped.
    pub const DEFAULT_CAPACITY: usize = 4096;

    /// Opens a cache rooted at `dir` with [`Self::DEFAULT_CAPACITY`],
    /// creating the directory (and its parents) if missing and pruning it to
    /// capacity.
    ///
    /// # Errors
    ///
    /// Returns the underlying [`io::Error`] when the directory cannot be
    /// created.
    pub fn open(dir: impl Into<PathBuf>) -> io::Result<Self> {
        Self::open_with_capacity(dir, Self::DEFAULT_CAPACITY)
    }

    /// Opens a cache rooted at `dir` keeping at most `capacity` entries.
    ///
    /// # Errors
    ///
    /// Returns the underlying [`io::Error`] when the directory cannot be
    /// created.
    pub fn open_with_capacity(dir: impl Into<PathBuf>, capacity: usize) -> io::Result<Self> {
        let dir = dir.into();
        fs::create_dir_all(&dir)?;
        prune::to_capacity(&dir, capacity);
        Ok(Self {
            dir,
            rules: 0
        })
    }

    /// Returns a handle that reads and writes entries under the rule-set
    /// fingerprint `rules_fingerprint`.
    #[must_use]
    pub fn under(&self, rules_fingerprint: &[u8]) -> Self {
        Self {
            dir:   self.dir.clone(),
            rules: rules_fingerprint.len()
        }
    }

    /// Looks up the entry for `key` and returns the findings of a file of
    /// `file_bytes` bytes, with every location rewritten to `file`.
    ///
    /// Returns `None` on a miss. An unreadable entry, a corrupt one, and one
    /// whose recorded input lengths do not match are all reported as misses,
    /// so the caller falls back to a fresh run whose store then repairs the
    /// entry.
    #[must_use]
    pub fn lookup(&self, key: &str, file: &Path, file_bytes: usize) -> Option<Vec<Finding>> {
        let path = self.entry_path(key);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return None,
            Err(err) => {
                warn!(entry = %path.display(), error = %err, "cache entry unreadable; miss");
                return None;
            }
        };
        let stored = match serde_json::from_slice::<Entry>(&bytes) {
            Ok(stored) => stored,
            Err(err) => {
                warn!(entry = %path.display(), error = %err, "corrupt cache entry; miss");
                return None;
            }
        };
        if !stored.describes(file_bytes, self.rules) {
            warn!(
                entry = %path.display(),
                file = %file.display(),
                "cache entry was built from different inputs; miss"
            );
            return None;
        }
        let mut findings = stored.findings;
        for finding in &mut findings {
            finding.location.file = file.to_path_buf();
        }
        debug!(entry = %path.display(), file = %file.display(), "cache hit");
        Some(findings)
    }

    /// Stores `findings` as the analysis of a file of `file_bytes` bytes
    /// under `key`, overwriting any existing entry.
    ///
    /// Failures are logged at `warn` level and never propagate: the cache is
    /// an optimization, not a correctness requirement.
    pub fn store(&self, key: &str, file_bytes: usize, findings: &[Finding]) {
        let path = self.entry_path(key);
        let entry = Entry::new(file_bytes, self.rules, findings.to_vec());
        match serde_json::to_vec(&entry) {
            Ok(bytes) => {
                if let Err(err) = fs::write(&path, bytes) {
                    warn!(entry = %path.display(), error = %err, "cache store failed");
                }
            }
            Err(err) => {
                warn!(entry = %path.display(), error = %err, "cache serialization failed");
            }
        }
    }

    /// Returns the on-disk path of the entry for `key`.
    fn entry_path(&self, key: &str) -> PathBuf {
        self.dir.join(format!("{key}.json"))
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use rustmanifest_schema::{Location, Range, Severity};
    use tempfile::tempdir;

    use super::*;

    /// Fingerprint every test writes and reads under, so an entry's
    /// recorded rule-set length matches unless a test changes it.
    const RULES: &[u8] = b"rules";

    fn cache(dir: &Path) -> FindingsCache {
        FindingsCache::open(dir).unwrap().under(RULES)
    }

    fn sample_finding(file: &str) -> Finding {
        Finding {
            rule_id:       "RM-AAA-001".to_owned(),
            severity:      Severity::Error,
            location:      Location {
                file:  PathBuf::from(file),
                range: Range {
                    start: 1, end: 5
                }
            },
            message:       "sample finding".to_owned(),
            rationale_uri: "rustmanifest://methodology/sample".to_owned(),
            trace:         Vec::new(),
            fixes:         Vec::new()
        }
    }

    #[test]
    fn store_then_lookup_round_trips_and_rewrites_path() {
        let dir = tempdir().unwrap();
        let cache = cache(dir.path());
        let stored = vec![sample_finding("original.rs")];
        cache.store("cafe", 12, &stored);

        let loaded = cache.lookup("cafe", Path::new("elsewhere.rs"), 12).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].location.file, PathBuf::from("elsewhere.rs"));
        assert_eq!(loaded[0].message, stored[0].message);
    }

    #[test]
    fn missing_entry_is_a_miss() {
        let dir = tempdir().unwrap();
        assert!(
            cache(dir.path())
                .lookup("0000", Path::new("any.rs"), 12)
                .is_none()
        );
    }

    #[test]
    fn corrupt_entry_is_a_miss_and_store_repairs_it() {
        let dir = tempdir().unwrap();
        let cache = cache(dir.path());
        fs::write(dir.path().join("dead.json"), b"not json at all").unwrap();
        assert!(cache.lookup("dead", Path::new("any.rs"), 12).is_none());

        cache.store("dead", 12, &[sample_finding("any.rs")]);
        assert_eq!(
            cache.lookup("dead", Path::new("any.rs"), 12).unwrap().len(),
            1
        );
    }

    /// The entry states the lengths it was built from, so an address that
    /// collides across differently sized inputs costs a re-analysis instead
    /// of answering about a file it never saw.
    #[test]
    fn an_entry_built_from_other_inputs_is_a_miss() {
        let dir = tempdir().unwrap();
        let cache = cache(dir.path());
        cache.store("cafe", 12, &[sample_finding("any.rs")]);

        assert!(cache.lookup("cafe", Path::new("any.rs"), 13).is_none());
        assert!(
            FindingsCache::open(dir.path())
                .unwrap()
                .under(b"other rules fingerprint")
                .lookup("cafe", Path::new("any.rs"), 12)
                .is_none()
        );
        assert!(cache.lookup("cafe", Path::new("any.rs"), 12).is_some());
    }

    #[test]
    fn a_findings_array_from_an_older_layout_is_a_miss() {
        let dir = tempdir().unwrap();
        let cache = cache(dir.path());
        let legacy = serde_json::to_vec(&[sample_finding("any.rs")]).unwrap();
        fs::write(dir.path().join("legacy.json"), legacy).unwrap();
        assert!(cache.lookup("legacy", Path::new("any.rs"), 12).is_none());
    }

    #[test]
    fn open_creates_nested_directories() {
        let dir = tempdir().unwrap();
        let nested = dir.path().join("a").join("b");
        FindingsCache::open(&nested).unwrap();
        assert!(nested.is_dir());
    }

    #[test]
    fn opening_a_full_store_prunes_it_to_capacity() {
        let dir = tempdir().unwrap();
        let cache = cache(dir.path());
        for index in 0..4 {
            cache.store(&format!("{index:032x}"), 12, &[sample_finding("any.rs")]);
        }
        FindingsCache::open_with_capacity(dir.path(), 2).unwrap();
        let remaining = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "json")
            })
            .count();
        assert_eq!(remaining, 2);
    }
}
