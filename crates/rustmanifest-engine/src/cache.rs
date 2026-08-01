// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Optional persistent findings cache, content-addressed per file.
//!
//! A [`FindingsCache`] is opened at a directory and stores one JSON file per
//! cache entry, named `<key>.json`, where the key is the hex-encoded FNV-1a
//! 64-bit hash of the concatenation of:
//!
//! - the raw bytes of the analyzed file,
//! - the sorted rule-id set plus every rule definition serialized as JSON (see
//!   [`rules_fingerprint`]),
//! - the engine crate version (`CARGO_PKG_VERSION`).
//!
//! An entry holds the serialized `Vec<Finding>` produced by a full
//! orchestrator pass over that file, **including** the engine-synthesized
//! `RM-META-001` unused-pragma findings, so a cache hit reproduces the fresh
//! run byte-for-byte. Because the key is derived from file content rather
//! than the file path, [`FindingsCache::lookup`] rewrites each cached
//! finding's location to the path being analyzed; identical files therefore
//! share one entry without leaking each other's paths.
//!
//! The workspace-level **semantic tier runs outside the orchestrator** (the
//! CLI drives it over the whole file set) and is deliberately **not** covered
//! by this cache: its findings depend on cross-file state that a per-file
//! content address cannot capture.
//!
//! Cache failures are never fatal: unreadable or corrupt entries are treated
//! as misses (and overwritten on the next store), and store failures are
//! logged at `warn` level while the run continues with fresh results.

use std::{
    fs, io,
    path::{Path, PathBuf}
};

use rustmanifest_schema::{Finding, Rule};
use tracing::{debug, warn};

/// FNV-1a 64-bit offset basis.
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;

/// FNV-1a 64-bit prime.
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Hashes `bytes` with the FNV-1a 64-bit algorithm.
fn fnv1a64(bytes: impl IntoIterator<Item = u8>) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    for byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Builds the rule-set half of the cache key.
///
/// The fingerprint is the sorted rule-id set, followed by every rule (sorted
/// by id) serialized as JSON, followed by the engine crate version. Each
/// component is NUL-separated so that adjacent components cannot collide by
/// concatenation.
///
/// A rule that fails to serialize contributes an empty component; the
/// fingerprint stays deterministic either way.
#[must_use]
pub fn rules_fingerprint(rules: &[&Rule]) -> Vec<u8> {
    let mut sorted: Vec<&Rule> = rules.to_vec();
    sorted.sort_unstable_by(|left, right| left.id.cmp(&right.id));
    let mut bytes = Vec::new();
    for rule in &sorted {
        bytes.extend_from_slice(rule.id.as_bytes());
        bytes.push(0);
    }
    for rule in &sorted {
        bytes.extend_from_slice(&serde_json::to_vec(rule).unwrap_or_default());
        bytes.push(0);
    }
    bytes.extend_from_slice(env!("CARGO_PKG_VERSION").as_bytes());
    bytes
}

/// Persistent, content-addressed store for per-file orchestrator findings.
///
/// See the [module documentation](self) for the key composition and the
/// failure model. Cloning is cheap; clones share the same directory.
#[derive(Debug, Clone)]
pub struct FindingsCache {
    dir: PathBuf
}

impl FindingsCache {
    /// Opens a cache rooted at `dir`, creating the directory (and its
    /// parents) if missing.
    ///
    /// # Errors
    ///
    /// Returns the underlying [`io::Error`] when the directory cannot be
    /// created.
    pub fn open(dir: impl Into<PathBuf>) -> io::Result<Self> {
        let dir = dir.into();
        fs::create_dir_all(&dir)?;
        Ok(Self {
            dir
        })
    }

    /// Returns the directory this cache stores entries in.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Computes the cache key for one file: the hex-encoded FNV-1a 64-bit
    /// hash of `file_bytes` followed by `rules_fingerprint` (as produced by
    /// [`rules_fingerprint`]).
    #[must_use]
    pub fn key(file_bytes: &[u8], rules_fingerprint: &[u8]) -> String {
        let hash = fnv1a64(file_bytes.iter().chain(rules_fingerprint).copied());
        format!("{hash:016x}")
    }

    /// Looks up the entry for `key` and returns its findings with every
    /// location rewritten to `file`.
    ///
    /// Returns `None` on a miss; unreadable or corrupt entries are logged at
    /// `warn` level and reported as misses so the caller falls back to a
    /// fresh run (whose store then repairs the entry).
    #[must_use]
    pub fn lookup(&self, key: &str, file: &Path) -> Option<Vec<Finding>> {
        let entry = self.entry_path(key);
        let bytes = match fs::read(&entry) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return None,
            Err(err) => {
                warn!(entry = %entry.display(), error = %err, "cache entry unreadable; miss");
                return None;
            }
        };
        match serde_json::from_slice::<Vec<Finding>>(&bytes) {
            Ok(mut findings) => {
                for finding in &mut findings {
                    finding.location.file = file.to_path_buf();
                }
                debug!(entry = %entry.display(), file = %file.display(), "cache hit");
                Some(findings)
            }
            Err(err) => {
                warn!(entry = %entry.display(), error = %err, "corrupt cache entry; miss");
                None
            }
        }
    }

    /// Stores `findings` under `key`, overwriting any existing entry.
    ///
    /// Failures are logged at `warn` level and never propagate: the cache is
    /// an optimization, not a correctness requirement.
    pub fn store(&self, key: &str, findings: &[Finding]) {
        let entry = self.entry_path(key);
        match serde_json::to_vec(findings) {
            Ok(bytes) => {
                if let Err(err) = fs::write(&entry, bytes) {
                    warn!(entry = %entry.display(), error = %err, "cache store failed");
                }
            }
            Err(err) => {
                warn!(entry = %entry.display(), error = %err, "cache serialization failed");
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
    use rustmanifest_schema::{Location, Range, RuleDefinition, Severity};
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn fnv1a64_matches_reference_vectors() {
        assert_eq!(fnv1a64([]), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64(*b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a64(*b"foobar"), 0x8594_4171_f739_67e8);
    }

    #[test]
    fn key_is_sixteen_hex_chars_and_input_sensitive() {
        let base = FindingsCache::key(b"contents", b"rules");
        assert_eq!(base.len(), 16);
        assert!(base.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_ne!(base, FindingsCache::key(b"contents!", b"rules"));
        assert_ne!(base, FindingsCache::key(b"contents", b"rules!"));
    }

    fn sample_rule(id: &str, regex: &str) -> Rule {
        Rule {
            id:            id.to_owned(),
            severity:      Severity::Error,
            title:         "sample".to_owned(),
            rationale_uri: "rustmanifest://methodology/sample".to_owned(),
            definition:    RuleDefinition::Pattern {
                regex:         regex.to_owned(),
                exclude_globs: vec![]
            }
        }
    }

    #[test]
    fn rules_fingerprint_is_order_independent_but_content_sensitive() {
        let first = sample_rule("RM-AAA-001", "foo");
        let second = sample_rule("RM-BBB-002", "bar");
        let forward = rules_fingerprint(&[&first, &second]);
        let backward = rules_fingerprint(&[&second, &first]);
        assert_eq!(forward, backward);

        let changed = sample_rule("RM-BBB-002", "baz");
        assert_ne!(forward, rules_fingerprint(&[&first, &changed]));
        assert_ne!(forward, rules_fingerprint(&[&first]));
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
        let cache = FindingsCache::open(dir.path()).unwrap();
        let stored = vec![sample_finding("original.rs")];
        cache.store("cafe", &stored);

        let loaded = cache.lookup("cafe", Path::new("elsewhere.rs")).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].location.file, PathBuf::from("elsewhere.rs"));
        assert_eq!(loaded[0].message, stored[0].message);
    }

    #[test]
    fn missing_entry_is_a_miss() {
        let dir = tempdir().unwrap();
        let cache = FindingsCache::open(dir.path()).unwrap();
        assert!(cache.lookup("0000", Path::new("any.rs")).is_none());
    }

    #[test]
    fn corrupt_entry_is_a_miss_and_store_repairs_it() {
        let dir = tempdir().unwrap();
        let cache = FindingsCache::open(dir.path()).unwrap();
        fs::write(dir.path().join("dead.json"), b"not json at all").unwrap();
        assert!(cache.lookup("dead", Path::new("any.rs")).is_none());

        cache.store("dead", &[sample_finding("any.rs")]);
        let repaired = cache.lookup("dead", Path::new("any.rs")).unwrap();
        assert_eq!(repaired.len(), 1);
    }

    #[test]
    fn open_creates_nested_directories() {
        let dir = tempdir().unwrap();
        let nested = dir.path().join("a").join("b");
        let cache = FindingsCache::open(&nested).unwrap();
        assert_eq!(cache.dir(), nested);
        assert!(nested.is_dir());
    }
}
