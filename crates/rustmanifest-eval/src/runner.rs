// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Corpus evaluation: runs the full engine composition over every pinned
//! repository and assembles a [`Report`].
//!
//! The composition mirrors the CLI's `check` path: every bundled pattern
//! and AST rule runs per-file through an orchestrator, and every bundled
//! semantic rule runs once per repository over all discovered sources.
//! Files are analyzed one at a time so each file's wall time can be
//! measured.

mod checkout;
mod composition;
mod repo;

use std::{collections::BTreeMap, path::Path};

pub use checkout::UNKNOWN_REV;
pub use repo::P95_LINE_LIMIT;

use crate::{corpus::Corpus, error::EvalError, report::Report};

/// Evaluates every corpus entry under `corpus_root` and assembles the full
/// report. Repositories are processed and reported in name order.
///
/// # Errors
///
/// Returns [`EvalError::MissingCheckout`] when a pinned checkout does not
/// exist and [`EvalError::Engine`] when analyzer construction or file
/// discovery fails. A drifted checkout revision is tolerated: it is logged
/// as a warning and recorded in the report.
pub fn evaluate(corpus: &Corpus, corpus_root: &Path) -> Result<Report, EvalError> {
    let orchestrator = composition::build_orchestrator()?;
    let semantic = composition::build_semantic()?;
    let mut repos = Vec::with_capacity(corpus.repos.len());
    let mut timings = BTreeMap::new();
    for entry in &corpus.repos {
        let (repo_report, repo_timings) =
            repo::evaluate(entry, corpus_root, &orchestrator, &semantic)?;
        timings.insert(repo_report.name.clone(), repo_timings);
        repos.push(repo_report);
    }
    repos.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(Report {
        engine_version: env!("CARGO_PKG_VERSION").to_owned(),
        repos,
        timings
    })
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
    use crate::corpus::RepoEntry;

    fn fixture_corpus(rev: &str) -> Corpus {
        Corpus {
            repos: vec![RepoEntry {
                name:  "fixture".to_owned(),
                path:  "fixture".to_owned(),
                rev:   rev.to_owned(),
                notes: "unit-test fixture".to_owned()
            }]
        }
    }

    fn write_fixture_tree(root: &Path) {
        let repo = root.join("fixture");
        std::fs::create_dir_all(repo.join("src")).unwrap();
        std::fs::write(
            repo.join("src/lib.rs"),
            "// rustmanifest: allow(RM-ZZZ-999) reason=\"stale fixture pragma\"\npub fn \
             answer() -> u32 {\n    42\n}\n"
        )
        .unwrap();
        std::fs::write(
            repo.join("src/other.rs"),
            "pub struct Plain {\n    pub id: u64\n}\n"
        )
        .unwrap();
    }

    #[test]
    fn two_runs_produce_identical_comparable_json() {
        let dir = tempfile::tempdir().unwrap();
        write_fixture_tree(dir.path());
        let corpus = fixture_corpus("0000000000000000000000000000000000000000");

        let first = evaluate(&corpus, dir.path()).unwrap();
        let second = evaluate(&corpus, dir.path()).unwrap();

        let first_json = serde_json::to_string_pretty(&first.comparable()).unwrap();
        let second_json = serde_json::to_string_pretty(&second.comparable()).unwrap();
        assert_eq!(first_json, second_json);
        assert_eq!(first.repos.len(), 1);
        assert_eq!(first.repos[0].files, 2);
        assert!(first.repos[0].rust_lines > 0);
        assert!(first.timings.contains_key("fixture"));
    }

    #[test]
    fn stale_pragma_fixture_yields_meta_finding() {
        let dir = tempfile::tempdir().unwrap();
        write_fixture_tree(dir.path());
        let corpus = fixture_corpus("0000000000000000000000000000000000000000");

        let report = evaluate(&corpus, dir.path()).unwrap();
        let meta = report.repos[0].findings.get("RM-META-001").copied();
        assert_eq!(meta, Some(1));
    }

    #[test]
    fn drifted_checkout_analyzes_the_pinned_revision() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(repo.join("src")).expect("dirs");
        let run = |args: &[&str]| {
            let output = std::process::Command::new("git")
                .arg("-C")
                .arg(&repo)
                .args(args)
                .env("GIT_AUTHOR_NAME", "eval")
                .env("GIT_AUTHOR_EMAIL", "eval@localhost")
                .env("GIT_COMMITTER_NAME", "eval")
                .env("GIT_COMMITTER_EMAIL", "eval@localhost")
                .output()
                .expect("git runs");
            assert!(output.status.success(), "git {args:?} failed");
            String::from_utf8_lossy(&output.stdout).trim().to_owned()
        };
        run(&["init", "-q"]);
        std::fs::write(repo.join("src/lib.rs"), "pub struct Config;\n").expect("write");
        run(&["add", "."]);
        run(&["commit", "-q", "-m", "pinned state"]);
        let pinned = run(&["rev-parse", "HEAD"]);
        std::fs::write(
            repo.join("src/lib.rs"),
            "pub struct Config;\npub struct Extra;\n"
        )
        .expect("write drift");
        run(&["add", "."]);
        run(&["commit", "-q", "-m", "drifted state"]);

        let manifest = format!(
            "[[repo]]\nname = \"probe\"\npath = \"repo\"\nrev = \"{pinned}\"\nnotes = \"t\"\n"
        );
        let corpus_file = dir.path().join("corpus.toml");
        std::fs::write(&corpus_file, manifest).expect("corpus");
        let corpus = crate::corpus::load(&corpus_file).expect("parse");
        let report = evaluate(&corpus, dir.path()).expect("report");
        assert_eq!(report.repos[0].actual_rev, pinned);
        assert_eq!(report.repos[0].rust_lines, 1);
    }

    #[test]
    fn rev_mismatch_is_tolerated_and_recorded() {
        let dir = tempfile::tempdir().unwrap();
        write_fixture_tree(dir.path());
        let pinned = "1111111111111111111111111111111111111111";
        let corpus = fixture_corpus(pinned);

        let report = evaluate(&corpus, dir.path()).unwrap();
        assert_eq!(report.repos[0].pinned_rev, pinned);
        assert_ne!(report.repos[0].actual_rev, pinned);
    }

    #[test]
    fn missing_checkout_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let corpus = fixture_corpus("0000000000000000000000000000000000000000");
        let err = evaluate(&corpus, dir.path()).unwrap_err();
        assert!(matches!(err, EvalError::MissingCheckout { .. }));
    }

    #[test]
    fn p95_counts_only_small_files() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("fixture");
        std::fs::create_dir_all(&repo).unwrap();
        let big = "pub const X: u32 = 0;\n".repeat(1200);
        std::fs::write(repo.join("big.rs"), big).unwrap();
        std::fs::write(repo.join("small.rs"), "pub fn tiny() {}\n").unwrap();
        let corpus = fixture_corpus("0000000000000000000000000000000000000000");

        let report = evaluate(&corpus, dir.path()).unwrap();
        let timings = report.timings.get("fixture").unwrap();
        assert_eq!(timings.files_under_1k, 1);
        assert_eq!(report.repos[0].files, 2);
    }
}
