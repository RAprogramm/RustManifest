// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Report shapes, the comparable section, baseline diffing, and the p95
//! percentile computation.
//!
//! A full [`Report`] contains everything one evaluation run produced.
//! The [`Comparable`] projection is the part diffed against the committed
//! baseline: timings are excluded because wall-clock numbers vary between
//! machines and runs, and the actual checkout revision is excluded because
//! local trees are allowed to drift from the pin.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Full evaluation report for one run over the corpus.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Report {
    /// Version of the engine workspace that produced the report.
    pub engine_version: String,
    /// Per-repository results, sorted by repository name.
    pub repos: Vec<RepoReport>,
    /// Per-repository wall-clock measurements, keyed by repository name.
    /// Never diffed against the baseline.
    pub timings: BTreeMap<String, RepoTimings>,
}

/// Per-repository findings section of a [`Report`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct RepoReport {
    /// Corpus entry name.
    pub name: String,
    /// Commit the corpus pins for this repository.
    pub pinned_rev: String,
    /// Commit actually checked out when the run happened.
    pub actual_rev: String,
    /// Number of Rust source files analyzed.
    pub files: u64,
    /// Total lines across all analyzed Rust sources.
    pub rust_lines: u64,
    /// Finding counts keyed by rule id, sorted by id.
    pub findings: BTreeMap<String, u64>,
}

/// Wall-clock measurements for one repository.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct RepoTimings {
    /// p95 per-file analysis time in milliseconds over files under 1000
    /// lines. Zero when no such file exists.
    pub p95_under_1k_ms: f64,
    /// Number of analyzed files under 1000 lines.
    pub files_under_1k: u64,
    /// Slowest single-file analysis time in milliseconds across all files.
    pub max_file_ms: f64,
    /// Wall time of the workspace-level semantic tier in milliseconds.
    pub semantic_ms: f64,
    /// Total wall time for the repository in milliseconds.
    pub total_ms: f64,
}

/// The baseline-comparable projection of a [`Report`]: everything except
/// timings and actual checkout revisions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Comparable {
    /// Version of the engine workspace that produced the report.
    pub engine_version: String,
    /// Per-repository comparable results, sorted by repository name.
    pub repos: Vec<ComparableRepo>,
}

/// Per-repository comparable section: [`RepoReport`] minus `actual-rev`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ComparableRepo {
    /// Corpus entry name.
    pub name: String,
    /// Commit the corpus pins for this repository.
    pub pinned_rev: String,
    /// Number of Rust source files analyzed.
    pub files: u64,
    /// Total lines across all analyzed Rust sources.
    pub rust_lines: u64,
    /// Finding counts keyed by rule id, sorted by id.
    pub findings: BTreeMap<String, u64>,
}

impl Report {
    /// Projects the report onto its baseline-comparable section.
    #[must_use]
    pub fn comparable(&self) -> Comparable {
        Comparable {
            engine_version: self.engine_version.clone(),
            repos: self
                .repos
                .iter()
                .map(|repo| ComparableRepo {
                    name: repo.name.clone(),
                    pinned_rev: repo.pinned_rev.clone(),
                    files: repo.files,
                    rust_lines: repo.rust_lines,
                    findings: repo.findings.clone(),
                })
                .collect(),
        }
    }
}

/// Computes the p95 of `samples` in milliseconds.
///
/// The percentile uses the nearest-rank method: for `n` samples the p95 is
/// the value at rank `ceil(0.95 * n)` (1-based) of the ascending-sorted
/// vector. An empty slice yields `0.0`.
#[must_use]
pub fn p95(samples: &[f64]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    let rank = (samples.len() * 95).div_ceil(100).max(1);
    sorted.get(rank - 1).copied().unwrap_or(0.0)
}

/// Lists every difference between the committed `baseline` and the
/// `current` comparable section. An empty result means the run reproduced
/// the baseline exactly.
#[must_use]
pub fn diff(baseline: &Comparable, current: &Comparable) -> Vec<String> {
    let mut differences = Vec::new();
    if baseline.engine_version != current.engine_version {
        differences.push(format!(
            "engine-version: baseline {}, current {}",
            baseline.engine_version, current.engine_version
        ));
    }

    let baseline_repos: BTreeMap<&str, &ComparableRepo> = baseline
        .repos
        .iter()
        .map(|repo| (repo.name.as_str(), repo))
        .collect();
    let current_repos: BTreeMap<&str, &ComparableRepo> = current
        .repos
        .iter()
        .map(|repo| (repo.name.as_str(), repo))
        .collect();

    for (name, repo) in &baseline_repos {
        match current_repos.get(name) {
            None => differences.push(format!("repo {name}: present in baseline, absent now")),
            Some(other) => diff_repo(repo, other, &mut differences),
        }
    }
    for name in current_repos.keys() {
        if !baseline_repos.contains_key(name) {
            differences.push(format!("repo {name}: absent from baseline, present now"));
        }
    }
    differences
}

/// Appends field-level differences between two repositories of the same
/// name onto `differences`.
fn diff_repo(baseline: &ComparableRepo, current: &ComparableRepo, differences: &mut Vec<String>) {
    let name = &baseline.name;
    if baseline.pinned_rev != current.pinned_rev {
        differences.push(format!(
            "repo {name} pinned-rev: baseline {}, current {}",
            baseline.pinned_rev, current.pinned_rev
        ));
    }
    if baseline.files != current.files {
        differences.push(format!(
            "repo {name} files: baseline {}, current {}",
            baseline.files, current.files
        ));
    }
    if baseline.rust_lines != current.rust_lines {
        differences.push(format!(
            "repo {name} rust-lines: baseline {}, current {}",
            baseline.rust_lines, current.rust_lines
        ));
    }
    let rule_ids: std::collections::BTreeSet<&String> = baseline
        .findings
        .keys()
        .chain(current.findings.keys())
        .collect();
    for rule_id in rule_ids {
        let before = baseline.findings.get(rule_id).copied().unwrap_or(0);
        let after = current.findings.get(rule_id).copied().unwrap_or(0);
        if before != after {
            differences.push(format!(
                "repo {name} rule {rule_id}: baseline {before}, current {after}"
            ));
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::float_cmp,
    reason = "test code is allowed to fail loudly on broken invariants and \
              compares exact float constants"
)]
mod tests {
    use super::*;

    fn repo(name: &str, findings: &[(&str, u64)]) -> ComparableRepo {
        ComparableRepo {
            name: name.to_owned(),
            pinned_rev: "abc".to_owned(),
            files: 2,
            rust_lines: 40,
            findings: findings
                .iter()
                .map(|(id, count)| ((*id).to_owned(), *count))
                .collect(),
        }
    }

    fn comparable(repos: Vec<ComparableRepo>) -> Comparable {
        Comparable {
            engine_version: "0.0.0".to_owned(),
            repos,
        }
    }

    #[test]
    fn p95_of_known_vector_is_nearest_rank() {
        let samples: Vec<f64> = (1..=100).map(f64::from).collect();
        assert_eq!(p95(&samples), 95.0);
    }

    #[test]
    fn p95_of_small_vectors() {
        assert_eq!(p95(&[]), 0.0);
        assert_eq!(p95(&[7.0]), 7.0);
        assert_eq!(p95(&[3.0, 1.0, 2.0]), 3.0);
        let twenty: Vec<f64> = (1..=20).map(f64::from).collect();
        assert_eq!(p95(&twenty), 19.0);
    }

    #[test]
    fn unchanged_reports_produce_no_diff() {
        let left = comparable(vec![repo("alpha", &[("RM-SEC-001", 3)])]);
        let right = left.clone();
        assert!(diff(&left, &right).is_empty());
    }

    #[test]
    fn changed_count_is_detected() {
        let baseline = comparable(vec![repo("alpha", &[("RM-SEC-001", 3)])]);
        let current = comparable(vec![repo("alpha", &[("RM-SEC-001", 5)])]);
        let differences = diff(&baseline, &current);
        assert_eq!(differences.len(), 1);
        assert_eq!(
            differences[0],
            "repo alpha rule RM-SEC-001: baseline 3, current 5"
        );
    }

    #[test]
    fn appearing_and_disappearing_rules_are_detected() {
        let baseline = comparable(vec![repo("alpha", &[("RM-SEC-001", 3)])]);
        let current = comparable(vec![repo("alpha", &[("RM-STRUCT-001", 1)])]);
        let differences = diff(&baseline, &current);
        assert_eq!(differences.len(), 2);
        assert!(differences[0].contains("rule RM-SEC-001: baseline 3, current 0"));
        assert!(differences[1].contains("rule RM-STRUCT-001: baseline 0, current 1"));
    }

    #[test]
    fn repo_set_changes_are_detected() {
        let baseline = comparable(vec![repo("alpha", &[])]);
        let current = comparable(vec![repo("beta", &[])]);
        let differences = diff(&baseline, &current);
        assert!(
            differences
                .iter()
                .any(|line| line == "repo alpha: present in baseline, absent now")
        );
        assert!(
            differences
                .iter()
                .any(|line| line == "repo beta: absent from baseline, present now")
        );
    }

    #[test]
    fn comparable_projection_drops_actual_rev_and_timings() {
        let report = Report {
            engine_version: "0.0.0".to_owned(),
            repos: vec![RepoReport {
                name: "alpha".to_owned(),
                pinned_rev: "abc".to_owned(),
                actual_rev: "drifted".to_owned(),
                files: 1,
                rust_lines: 10,
                findings: BTreeMap::from([("RM-SEC-001".to_owned(), 2)]),
            }],
            timings: BTreeMap::from([(
                "alpha".to_owned(),
                RepoTimings {
                    p95_under_1k_ms: 1.0,
                    files_under_1k: 1,
                    max_file_ms: 2.0,
                    semantic_ms: 3.0,
                    total_ms: 4.0,
                },
            )]),
        };
        let json = serde_json::to_string(&report.comparable()).unwrap();
        assert!(!json.contains("actual-rev"));
        assert!(!json.contains("timings"));
        assert!(json.contains("\"pinned-rev\":\"abc\""));
    }
}
