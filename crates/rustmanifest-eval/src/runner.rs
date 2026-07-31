// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Corpus evaluation: runs the full engine composition over every pinned
//! repository and assembles a [`Report`].
//!
//! The composition mirrors the CLI's `check` path: every bundled pattern
//! and AST rule runs per-file through an [`Orchestrator`], and every
//! bundled semantic rule runs once per repository over all discovered
//! sources. Files are analyzed one at a time so each file's wall time can
//! be measured with [`Instant`].

use std::{collections::BTreeMap, path::Path, process::Command, time::Instant};

use rustmanifest_engine::{
    Analyzer, AstAnalyzer, Orchestrator, OrchestratorBuilder, PatternAnalyzer, SemanticAnalyzer,
    Source, orchestrator::DEFAULT_MAX_FILE_BYTES, walker
};
use rustmanifest_rules_core::RULES;
use rustmanifest_schema::RuleDefinition;
use tracing::warn;

use crate::{
    corpus::{Corpus, RepoEntry},
    error::EvalError,
    report::{RepoReport, RepoTimings, Report, p95}
};

/// Line-count threshold under which a file participates in the p95
/// performance budget.
pub const P95_LINE_LIMIT: usize = 1000;

/// Revision recorded when `git rev-parse HEAD` cannot be resolved.
pub const UNKNOWN_REV: &str = "unknown";

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
    let orchestrator = build_orchestrator()?;
    let semantic = build_semantic()?;
    let mut repos = Vec::with_capacity(corpus.repos.len());
    let mut timings = BTreeMap::new();
    for entry in &corpus.repos {
        let (repo_report, repo_timings) =
            evaluate_repo(entry, corpus_root, &orchestrator, &semantic)?;
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

/// Builds one orchestrator holding every bundled pattern and AST rule.
fn build_orchestrator() -> Result<Orchestrator, EvalError> {
    let analyzers: Vec<Box<dyn Analyzer>> = RULES
        .iter()
        .filter_map(|rule| match rule.definition {
            RuleDefinition::Pattern {
                ..
            } => Some(
                PatternAnalyzer::new(rule.clone())
                    .map(|analyzer| Box::new(analyzer) as Box<dyn Analyzer>)
            ),
            RuleDefinition::Ast {
                ..
            } => Some(
                AstAnalyzer::new(rule.clone())
                    .map(|analyzer| Box::new(analyzer) as Box<dyn Analyzer>)
            ),
            RuleDefinition::Semantic {
                ..
            } => None
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(OrchestratorBuilder::new().analyzers(analyzers).build())
}

/// Builds one analyzer per bundled semantic rule.
fn build_semantic() -> Result<Vec<SemanticAnalyzer>, EvalError> {
    Ok(RULES
        .iter()
        .filter(|rule| matches!(rule.definition, RuleDefinition::Semantic { .. }))
        .map(|rule| SemanticAnalyzer::new(rule.clone()))
        .collect::<Result<Vec<_>, _>>()?)
}

/// Resolves the checked-out revision of `path`, falling back to
/// [`UNKNOWN_REV`] when the directory is not a resolvable git checkout.
fn head_revision(path: &Path) -> String {
    Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map_or_else(
            || UNKNOWN_REV.to_owned(),
            |output| String::from_utf8_lossy(&output.stdout).trim().to_owned()
        )
}

/// Runs the engine over one repository checkout and measures per-file wall
/// time.
fn evaluate_repo(
    entry: &RepoEntry,
    corpus_root: &Path,
    orchestrator: &Orchestrator,
    semantic: &[SemanticAnalyzer]
) -> Result<(RepoReport, RepoTimings), EvalError> {
    let repo_path = corpus_root.join(&entry.path);
    if !repo_path.is_dir() {
        return Err(EvalError::MissingCheckout {
            name: entry.name.clone(),
            path: repo_path
        });
    }
    let actual_rev = head_revision(&repo_path);
    let mut materialized: Option<tempfile::TempDir> = None;
    let analyzed_rev;
    let analyzed_path = if actual_rev == entry.rev {
        analyzed_rev = entry.rev.clone();
        repo_path
    } else if let Some(pinned_tree) = materialize_rev(&repo_path, &entry.rev) {
        warn!(
            repo = %entry.name,
            pinned = %entry.rev,
            actual = %actual_rev,
            "live checkout drifted; analyzing the pinned revision from git archive"
        );
        analyzed_rev = entry.rev.clone();
        let path = pinned_tree.path().to_path_buf();
        materialized = Some(pinned_tree);
        path
    } else {
        warn!(
            repo = %entry.name,
            pinned = %entry.rev,
            actual = %actual_rev,
            "pinned revision not materializable; analyzing the live tree, counts may not be \
             comparable"
        );
        analyzed_rev = actual_rev;
        repo_path
    };

    let repo_start = Instant::now();
    let files = walker::discover(std::slice::from_ref(&analyzed_path))?;
    let mut findings: BTreeMap<String, u64> = BTreeMap::new();
    let mut budget_samples: Vec<f64> = Vec::new();
    let mut max_file_ms = 0.0_f64;
    let mut rust_lines = 0_u64;
    let mut sources: Vec<Source> = Vec::new();

    for file in &files {
        let Ok(text) = std::fs::read_to_string(file) else {
            warn!(path = %file.display(), "unreadable file skipped");
            continue;
        };
        let lines = text.lines().count();
        rust_lines += u64::try_from(lines).unwrap_or(u64::MAX);

        let file_start = Instant::now();
        let file_findings = orchestrator.run(std::slice::from_ref(file))?;
        let elapsed_ms = file_start.elapsed().as_secs_f64() * 1000.0;
        if lines < P95_LINE_LIMIT {
            budget_samples.push(elapsed_ms);
        }
        max_file_ms = max_file_ms.max(elapsed_ms);
        for finding in file_findings {
            *findings.entry(finding.rule_id).or_insert(0) += 1;
        }
        if u64::try_from(text.len()).unwrap_or(u64::MAX) <= DEFAULT_MAX_FILE_BYTES {
            sources.push(Source::new(file.clone(), text));
        }
    }

    let semantic_start = Instant::now();
    for analyzer in semantic {
        for finding in analyzer.analyze(&sources) {
            *findings.entry(finding.rule_id).or_insert(0) += 1;
        }
    }
    let semantic_ms = semantic_start.elapsed().as_secs_f64() * 1000.0;

    let timings = RepoTimings {
        p95_under_1k_ms: p95(&budget_samples),
        files_under_1k: u64::try_from(budget_samples.len()).unwrap_or(u64::MAX),
        max_file_ms,
        semantic_ms,
        total_ms: repo_start.elapsed().as_secs_f64() * 1000.0
    };
    let report = RepoReport {
        name: entry.name.clone(),
        pinned_rev: entry.rev.clone(),
        actual_rev: analyzed_rev,
        files: u64::try_from(files.len()).unwrap_or(u64::MAX),
        rust_lines,
        findings
    };
    drop(materialized);
    Ok((report, timings))
}

/// Extracts `rev` from the repository at `path` into a temporary directory
/// via `git archive`, returning `None` when the revision is unavailable.
fn materialize_rev(path: &Path, rev: &str) -> Option<tempfile::TempDir> {
    let dir = tempfile::TempDir::new().ok()?;
    let archive = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["archive", "--format=tar", rev])
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    let mut child = Command::new("tar")
        .arg("-x")
        .arg("-C")
        .arg(dir.path())
        .stdin(std::process::Stdio::piped())
        .spawn()
        .ok()?;
    {
        use std::io::Write;

        let stdin = child.stdin.as_mut()?;
        stdin.write_all(&archive.stdout).ok()?;
    }
    child
        .wait()
        .ok()
        .filter(std::process::ExitStatus::success)?;
    Some(dir)
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
