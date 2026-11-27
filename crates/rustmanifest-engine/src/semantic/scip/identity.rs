// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Identity of one analyzed source file for collision settling: which
//! crate it belongs to and which module path it holds, derived from the
//! file path and the module graph. Inline submodules share the file's
//! primary module — a documented approximation; a collision the
//! identity cannot settle is dropped, so the approximation can only
//! withhold an edge, never invent one.

use super::symbol::Callee;
use crate::{semantic::module_graph::ModuleGraph, source::Source};

/// Crate and module identity of one source file.
pub(super) struct SourceIdentity {
    /// Crate directory name from the file path, version suffix kept.
    crate_name: String,
    /// Module path segments below the crate root.
    modules:    Vec<String>
}

impl SourceIdentity {
    /// Returns whether this file can own the callee's symbol under
    /// the given short function name.
    pub(super) fn matches(&self, callee: &Callee, function: &str) -> bool {
        if !crate_matches(&self.crate_name, &callee.crate_name) {
            return false;
        }
        let mut full = self.modules.clone();
        full.extend(function.split("::").map(str::to_owned));
        suffix_aligned(&full, &callee.segments)
    }
}

/// Builds the identity of every source: the crate from the path,
/// the module path from the graph's primary module for the file.
pub(super) fn of_sources(
    sources: &[Source],
    settled: Option<&rustmanifest_schema::Resolution>
) -> Vec<SourceIdentity> {
    let graph = ModuleGraph::of(sources, settled);
    let mut modules: Vec<Option<Vec<String>>> = vec![None; sources.len()];
    for module in graph.modules() {
        let segments: Vec<String> = module.path.split("::").skip(1).map(str::to_owned).collect();
        let slot = &mut modules[module.source];
        let replace = slot.as_ref().is_none_or(|kept| segments.len() < kept.len());
        if replace {
            *slot = Some(segments);
        }
    }
    sources
        .iter()
        .zip(modules)
        .map(|(source, module)| SourceIdentity {
            crate_name: crate_of_path(&source.path().to_string_lossy()),
            modules:    module.unwrap_or_default()
        })
        .collect()
}

/// Crate directory name of one source path: the path component
/// preceding its `src` directory, or the file's parent directory.
fn crate_of_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    let components: Vec<&str> = normalized.split('/').filter(|c| !c.is_empty()).collect();
    components
        .iter()
        .position(|component| *component == "src")
        .and_then(|position| position.checked_sub(1))
        .and_then(|index| components.get(index))
        .or_else(|| {
            components
                .len()
                .checked_sub(2)
                .and_then(|i| components.get(i))
        })
        .map_or_default(|name| (*name).to_owned())
}

/// Returns whether a crate directory name names the symbol's crate,
/// tolerating a registry `-<version>` suffix and dash/underscore
/// spelling differences.
fn crate_matches(directory: &str, symbol_crate: &str) -> bool {
    let stripped = directory
        .rfind('-')
        .filter(|&split| {
            directory[split + 1..]
                .chars()
                .next()
                .is_some_and(|first| first.is_ascii_digit())
        })
        .map_or(directory, |split| &directory[..split]);
    stripped.replace('-', "_") == symbol_crate.replace('-', "_")
}

/// Returns whether the shorter of the two segment lists equals the
/// tail of the longer one.
fn suffix_aligned(ours: &[String], theirs: &[String]) -> bool {
    let overlap = ours.len().min(theirs.len());
    overlap > 0 && ours[ours.len() - overlap..] == theirs[theirs.len() - overlap..]
}
