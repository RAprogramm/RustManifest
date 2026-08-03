// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Compact per-repository summary every subcommand prints for a report.

use std::io::Write;

use crate::{error::EvalError, report::Report};

/// Writes the compact per-repository summary onto `handle`.
pub(super) fn write_summary(handle: &mut impl Write, report: &Report) -> Result<(), EvalError> {
    for repo in &report.repos {
        let total: u64 = repo.findings.values().sum();
        let p95 = report
            .timings
            .get(&repo.name)
            .map_or(0.0, |timings| timings.p95_under_1k_ms);
        writeln!(
            handle,
            "{}: {} files, {} lines, {} finding(s), p95 {:.2} ms",
            repo.name, repo.files, repo.rust_lines, total, p95
        )?;
    }
    Ok(())
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
    fn summary_writes_one_line_per_repo() {
        let mut buffer = Vec::new();
        write_summary(&mut buffer, &report_with_p95(1.0)).unwrap();
        let text = String::from_utf8(buffer).unwrap();
        assert_eq!(
            text,
            "alpha: 1 files, 10 lines, 0 finding(s), p95 1.00 ms\n"
        );
    }
}
