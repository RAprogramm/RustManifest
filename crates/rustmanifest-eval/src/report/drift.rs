// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Diffing of the comparable section against the committed baseline.

use std::collections::{BTreeMap, BTreeSet};

use super::model::{Comparable, ComparableRepo};

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
            Some(other) => diff_repo(repo, other, &mut differences)
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
    let rule_ids: BTreeSet<&String> = baseline
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
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use super::*;

    fn repo(name: &str, findings: &[(&str, u64)]) -> ComparableRepo {
        ComparableRepo {
            name:       name.to_owned(),
            pinned_rev: "abc".to_owned(),
            files:      2,
            rust_lines: 40,
            findings:   findings
                .iter()
                .map(|(id, count)| ((*id).to_owned(), *count))
                .collect()
        }
    }

    fn comparable(repos: Vec<ComparableRepo>) -> Comparable {
        Comparable {
            engine_version: "0.0.0".to_owned(),
            repos
        }
    }

    #[test]
    fn unchanged_reports_produce_no_diff() {
        let left = comparable(vec![repo("alpha", &[("RM-SEC-001", 3)])]);
        let right = left.clone();
        assert_eq!(diff(&left, &right), [] as [std::string::String; 0]);
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
}
