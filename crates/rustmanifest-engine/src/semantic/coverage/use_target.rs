// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Where the `use` targets of a module slice point.
//!
//! A target either resolves to a module of the same crate, or its leading
//! segment names a known external crate — the standard-library family or a
//! declared dependency — or it lands in the honest "did not understand"
//! bucket. One three-way decision backs both the counters of a measurement
//! and the inspectable listing of what was not understood, so the two can
//! never disagree.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    semantic::module_graph::{ModuleGraph, ModuleNode, resolved_module},
    source::Source
};

/// Three-way split of the `use` targets of one module slice.
pub(super) struct UseTargets {
    /// Expanded `use` targets across all modules.
    pub(super) total:      u64,
    /// Targets that resolve to a module of the same crate.
    pub(super) resolved:   u64,
    /// Targets whose leading segment names a known external crate.
    pub(super) external:   u64,
    /// Targets that neither resolve in-workspace nor name a known external
    /// crate.
    pub(super) unresolved: u64
}

/// Splits every `use` target declared by `modules` into resolved, external,
/// and unresolved.
pub(super) fn use_targets(
    modules: &[ModuleNode],
    sources: &[Source],
    settled: Option<&rustmanifest_schema::Resolution>,
    external_crates: &BTreeSet<String>
) -> UseTargets {
    let mut total = 0u64;
    let mut resolved = 0u64;
    let mut external = 0u64;
    let mut unresolved = 0u64;
    for (from, module) in modules.iter().enumerate() {
        for use_target in &module.uses {
            total += 1;
            if resolved_module(modules, sources, settled, from, use_target).is_some() {
                resolved += 1;
            } else if external_target(&use_target.target, external_crates) {
                external += 1;
            } else {
                unresolved += 1;
            }
        }
    }
    UseTargets {
        total,
        resolved,
        external,
        unresolved
    }
}

/// Lists the `use` targets [`coverage_with`] counts as unresolved — not
/// resolvable in-workspace and not attributable to a known external
/// crate — keyed by target path with occurrence counts.
///
/// This is the inspectable form of the coverage report's "unresolved"
/// bucket: it shows exactly which import shapes the engine does not yet
/// understand.
///
/// [`coverage_with`]: super::coverage_with
#[must_use]
pub fn unresolved_targets(
    sources: &[Source],
    external_crates: &BTreeSet<String>,
    settled: Option<&rustmanifest_schema::Resolution>
) -> BTreeMap<String, u64> {
    let graph = ModuleGraph::of(sources, settled);
    let modules = graph.modules();
    let mut targets = BTreeMap::new();
    for (from, module) in modules.iter().enumerate() {
        for use_target in &module.uses {
            if resolved_module(modules, sources, settled, from, use_target).is_none()
                && !external_target(&use_target.target, external_crates)
            {
                *targets.entry(use_target.target.clone()).or_default() += 1;
            }
        }
    }
    targets
}

/// Returns whether a `use` target's leading segment names a known external
/// crate: the standard-library family or a declared dependency.
fn external_target(target: &str, external_crates: &BTreeSet<String>) -> bool {
    const BUILTIN: [&str; 5] = ["alloc", "core", "proc_macro", "std", "test"];
    let first = target.split("::").next().unwrap_or(target);
    BUILTIN.contains(&first) || external_crates.contains(first)
}
