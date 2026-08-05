// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Single-repository evaluation: discovers the sources of one checkout,
//! runs the per-file and repository-wide analyzers over them and collects
//! the finding counts and wall times of that repository.

use std::{collections::BTreeMap, path::Path, time::Instant};

use rustmanifest_engine::{
    Orchestrator, SemanticAnalyzer, Source, orchestrator::DEFAULT_MAX_FILE_BYTES, walker
};
use tracing::warn;

use super::checkout::{self, AnalyzedTree};
use crate::{
    corpus::RepoEntry,
    error::EvalError,
    report::{RepoReport, RepoTimings, p95}
};

/// Line-count threshold under which a file participates in the p95
/// performance budget.
pub const P95_LINE_LIMIT: usize = 1000;

/// Runs the engine over one repository checkout and measures per-file wall
/// time.
///
/// # Errors
///
/// Returns [`EvalError::MissingCheckout`] when the pinned checkout does not
/// exist and [`EvalError::Engine`] when file discovery or analysis fails.
pub(super) fn evaluate(
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
    let AnalyzedTree {
        path: analyzed_path,
        rev: analyzed_rev,
        materialized
    } = checkout::select(entry, repo_path);

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
    let workspace = rustmanifest_engine::Workspace::new(&sources);
    for finding in rustmanifest_engine::semantic_findings(semantic, &workspace) {
        *findings.entry(finding.rule_id).or_insert(0) += 1;
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
