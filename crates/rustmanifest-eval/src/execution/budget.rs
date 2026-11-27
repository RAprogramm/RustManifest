// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Per-file latency budget and the breaches a report shows against it.

use crate::report::Report;

/// Per-file p95 budget in milliseconds for files under 1000 lines.
pub const P95_BUDGET_MS: f64 = 100.0;

/// Lists every repository whose p95 per-file time over files under 1000
/// lines exceeds [`P95_BUDGET_MS`].
pub(super) fn budget_breaches(report: &Report) -> Vec<String> {
    report
        .timings
        .iter()
        .filter(|(_, timings)| timings.p95_under_1k_ms > P95_BUDGET_MS)
        .map(|(name, timings)| {
            format!(
                "repo {name}: p95 {:.2} ms over files under 1k lines exceeds {P95_BUDGET_MS} ms",
                timings.p95_under_1k_ms
            )
        })
        .collect()
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

    fn report_with_p95(p95: f64) -> Report {
        Report {
            engine_version: "0.0.0".to_owned(),
            repos:          vec![RepoReport {
                name:       "alpha".to_owned(),
                pinned_rev: "abc".to_owned(),
                actual_rev: "abc".to_owned(),
                files:      1,
                rust_lines: 10,
                findings:   BTreeMap::new()
            }],
            timings:        BTreeMap::from([(
                "alpha".to_owned(),
                RepoTimings {
                    p95_under_1k_ms: p95,
                    files_under_1k:  1,
                    max_file_ms:     p95,
                    semantic_ms:     0.0,
                    total_ms:        p95
                }
            )])
        }
    }

    #[test]
    fn budget_breach_is_reported_above_the_limit() {
        let breaches = budget_breaches(&report_with_p95(100.5));
        assert_eq!(breaches.len(), 1);
        assert!(breaches[0].contains("repo alpha"));
        assert!(breaches[0].contains("100.50 ms"));
    }

    #[test]
    fn budget_holds_at_or_below_the_limit() {
        assert_eq!(
            budget_breaches(&report_with_p95(100.0)),
            [] as [std::string::String; 0]
        );
        assert_eq!(
            budget_breaches(&report_with_p95(3.0)),
            [] as [std::string::String; 0]
        );
    }
}
