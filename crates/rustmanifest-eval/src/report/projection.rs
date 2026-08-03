// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Projection of a full report onto its baseline-comparable section.

use super::model::{Comparable, ComparableRepo, Report};

impl Report {
    /// Projects the report onto its baseline-comparable section.
    #[must_use]
    pub fn comparable(&self) -> Comparable {
        Comparable {
            engine_version: self.engine_version.clone(),
            repos:          self
                .repos
                .iter()
                .map(|repo| ComparableRepo {
                    name:       repo.name.clone(),
                    pinned_rev: repo.pinned_rev.clone(),
                    files:      repo.files,
                    rust_lines: repo.rust_lines,
                    findings:   repo.findings.clone()
                })
                .collect()
        }
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
    use std::collections::BTreeMap;

    use super::*;
    use crate::report::{RepoReport, RepoTimings};

    #[test]
    fn comparable_projection_drops_actual_rev_and_timings() {
        let report = Report {
            engine_version: "0.0.0".to_owned(),
            repos:          vec![RepoReport {
                name:       "alpha".to_owned(),
                pinned_rev: "abc".to_owned(),
                actual_rev: "drifted".to_owned(),
                files:      1,
                rust_lines: 10,
                findings:   BTreeMap::from([("RM-SEC-001".to_owned(), 2)])
            }],
            timings:        BTreeMap::from([(
                "alpha".to_owned(),
                RepoTimings {
                    p95_under_1k_ms: 1.0,
                    files_under_1k:  1,
                    max_file_ms:     2.0,
                    semantic_ms:     3.0,
                    total_ms:        4.0
                }
            )])
        };
        let json = serde_json::to_string(&report.comparable()).unwrap();
        assert!(!json.contains("actual-rev"));
        assert!(!json.contains("timings"));
        assert!(json.contains("\"pinned-rev\":\"abc\""));
    }
}
