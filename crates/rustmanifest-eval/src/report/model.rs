// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The report data model: the full report, its per-repository sections, the
//! wall-clock measurements, and the baseline-comparable projection shapes.
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
    pub repos:          Vec<RepoReport>,
    /// Per-repository wall-clock measurements, keyed by repository name.
    /// Never diffed against the baseline.
    pub timings:        BTreeMap<String, RepoTimings>
}

/// Per-repository findings section of a [`Report`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct RepoReport {
    /// Corpus entry name.
    pub name:       String,
    /// Commit the corpus pins for this repository.
    pub pinned_rev: String,
    /// Commit actually checked out when the run happened.
    pub actual_rev: String,
    /// Number of Rust source files analyzed.
    pub files:      u64,
    /// Total lines across all analyzed Rust sources.
    pub rust_lines: u64,
    /// Finding counts keyed by rule id, sorted by id.
    pub findings:   BTreeMap<String, u64>
}

/// Wall-clock measurements for one repository.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct RepoTimings {
    /// p95 per-file analysis time in milliseconds over files under 1000
    /// lines. Zero when no such file exists.
    pub p95_under_1k_ms: f64,
    /// Number of analyzed files under 1000 lines.
    pub files_under_1k:  u64,
    /// Slowest single-file analysis time in milliseconds across all files.
    pub max_file_ms:     f64,
    /// Wall time of the workspace-level semantic tier in milliseconds.
    pub semantic_ms:     f64,
    /// Total wall time for the repository in milliseconds.
    pub total_ms:        f64
}

/// The baseline-comparable projection of a [`Report`]: everything except
/// timings and actual checkout revisions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Comparable {
    /// Version of the engine workspace that produced the report.
    pub engine_version: String,
    /// Per-repository comparable results, sorted by repository name.
    pub repos:          Vec<ComparableRepo>
}

/// Per-repository comparable section: [`RepoReport`] minus `actual-rev`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ComparableRepo {
    /// Corpus entry name.
    pub name:       String,
    /// Commit the corpus pins for this repository.
    pub pinned_rev: String,
    /// Number of Rust source files analyzed.
    pub files:      u64,
    /// Total lines across all analyzed Rust sources.
    pub rust_lines: u64,
    /// Finding counts keyed by rule id, sorted by id.
    pub findings:   BTreeMap<String, u64>
}
