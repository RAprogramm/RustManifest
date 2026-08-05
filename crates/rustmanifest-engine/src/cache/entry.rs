// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What one cache entry holds, and what it states about its own inputs.
//!
//! An entry records the byte lengths it was built from beside the findings.
//! The key already addresses the content, so the lengths add nothing on the
//! happy path — they exist so that an entry reached under a colliding or
//! stale key is recognised and refused rather than served as an answer about
//! a file it never saw.

use rustmanifest_schema::Finding;
use serde::{Deserialize, Serialize};

/// One stored analysis of one file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(super) struct Entry {
    /// Byte length of the analyzed file.
    pub(super) file_bytes:  u64,
    /// Byte length of the rule-set fingerprint the analysis ran under.
    pub(super) rules_bytes: u64,
    /// Findings the orchestrator produced for that file under those rules.
    pub(super) findings:    Vec<Finding>
}

impl Entry {
    /// Records `findings` as the analysis of a file of `file_bytes` bytes
    /// under a fingerprint of `rules_bytes` bytes.
    pub(super) fn new(file_bytes: usize, rules_bytes: usize, findings: Vec<Finding>) -> Self {
        Self {
            file_bytes: length(file_bytes),
            rules_bytes: length(rules_bytes),
            findings
        }
    }

    /// Returns whether this entry was built from inputs of exactly these
    /// lengths.
    pub(super) fn describes(&self, file_bytes: usize, rules_bytes: usize) -> bool {
        self.file_bytes == length(file_bytes) && self.rules_bytes == length(rules_bytes)
    }
}

/// Records a byte length, saturating rather than wrapping on a platform
/// where a `usize` does not fit a `u64`.
fn length(bytes: usize) -> u64 {
    u64::try_from(bytes).unwrap_or(u64::MAX)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use super::*;

    #[test]
    fn an_entry_describes_the_lengths_it_was_built_from() {
        let entry = Entry::new(120, 40, Vec::new());
        assert!(entry.describes(120, 40));
        assert!(!entry.describes(121, 40));
        assert!(!entry.describes(120, 41));
    }

    #[test]
    fn an_entry_round_trips_through_json() {
        let entry = Entry::new(7, 3, Vec::new());
        let bytes = serde_json::to_vec(&entry).unwrap();
        assert_eq!(serde_json::from_slice::<Entry>(&bytes).unwrap(), entry);
    }

    #[test]
    fn a_bare_findings_array_is_not_an_entry() {
        assert!(serde_json::from_slice::<Entry>(b"[]").is_err());
    }
}
