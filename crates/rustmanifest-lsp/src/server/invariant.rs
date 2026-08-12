// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Workspace invariants as diagnostics: the declared constraints
//! enforced over the whole snapshot, published where their violations
//! stand.
//!
//! The invariants come from the root's own `rustmanifest.toml` — the
//! same declarations the batch check enforces — and are judged over
//! the snapshot's already-derived facts, call graph, and transitive
//! effects, so a settled document event pays for no derivation twice.
//! Each violation lands as a diagnostic on the violating function in
//! the published document, its message carrying the full derivation
//! trace: the finding's authority is the trace, and the surface
//! carries it whole. A missing or unparsable configuration publishes
//! nothing — the batch check is the surface that reports a broken
//! configuration as an error.

use std::path::Path;

use lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString, Range};
use rustmanifest_schema::{Finding, Severity};

use super::{position::LineIndex, snapshot::Derived};

/// The invariant diagnostics of one published document: every declared
/// invariant whose violation stands in it.
pub(super) fn diagnostics(
    held: &Derived<'_>,
    root: Option<&Path>,
    path: &Path
) -> Vec<Diagnostic> {
    let declared = declared_invariants(root);
    if declared.is_empty() {
        return Vec::new();
    }
    let Some(file) = held.sources.iter().position(|source| source.path() == path) else {
        return Vec::new();
    };
    let text = held.sources[file].text();
    let index = LineIndex::new(text);
    rustmanifest_engine::semantic::check_invariants_over(
        held.sources,
        held.facts,
        held.graph,
        held.reached,
        &declared,
        held.settled
    )
    .into_iter()
    .filter(|finding| finding.location.file == path)
    .map(|finding| Diagnostic {
        range: Range {
            start: index.position(text, finding.location.range.start as usize),
            end:   index.position(text, finding.location.range.end as usize)
        },
        severity: Some(graded(finding.severity)),
        code: Some(NumberOrString::String(finding.rule_id.clone())),
        source: Some("rust-manifest".to_owned()),
        message: traced(&finding),
        ..Diagnostic::default()
    })
    .collect()
}

/// The invariants the workspace root declares; none where no root was
/// named, no configuration file exists, or it does not parse.
fn declared_invariants(root: Option<&Path>) -> Vec<rustmanifest_schema::Invariant> {
    let Some(file) = root.map(|dir| dir.join("rustmanifest.toml")) else {
        return Vec::new();
    };
    if !file.is_file() {
        return Vec::new();
    }
    rustmanifest_config::load(Some(&file))
        .ok()
        .and_then(|config| rustmanifest_config::Effective::from_config(config).ok())
        .map(|effective| effective.invariants().to_vec())
        .unwrap_or_default()
}

/// The finding's message with its derivation appended, one trace step
/// per line, fact and reason both.
fn traced(finding: &Finding) -> String {
    let mut message = finding.message.clone();
    for step in &finding.trace {
        message.push_str("\n• ");
        message.push_str(&step.fact);
        message.push_str(" — ");
        message.push_str(&step.reason);
    }
    message
}

/// The protocol's grade of a finding's severity.
const fn graded(severity: Severity) -> DiagnosticSeverity {
    match severity {
        Severity::Error => DiagnosticSeverity::ERROR,
        Severity::Warning => DiagnosticSeverity::WARNING,
        Severity::Info => DiagnosticSeverity::INFORMATION,
        Severity::Hint => DiagnosticSeverity::HINT
    }
}
