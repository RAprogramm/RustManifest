// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The dependency edges of the module graph, derived from `use` targets.
//!
//! An edge from module A to module B exists when a `use` target written in
//! A resolves to B within the same crate. External and unresolvable
//! targets produce no edge, and neither do self-imports. Each module pair
//! keeps its first use target as evidence.

use crate::semantic::module_graph::{ModuleGraph, resolved_module};

/// A resolved module-to-module edge with the use target it came from.
#[derive(Debug, Clone, Copy)]
pub(in crate::semantic) struct UseEdge {
    /// Index of the target module.
    pub(in crate::semantic) to:        usize,
    /// Index into the source module's `uses` list, kept as evidence.
    pub(in crate::semantic) use_index: usize
}

/// Builds per-module dependency edges from the expanded `use` targets.
///
/// Edges are deduplicated per module pair, keeping the first use target as
/// evidence, and self-edges are dropped.
pub(in crate::semantic) fn dependency_edges(
    graph: &ModuleGraph,
    sources: &[crate::source::Source],
    settled: Option<&rustmanifest_schema::Resolution>
) -> Vec<Vec<UseEdge>> {
    let modules = graph.modules();
    let mut edges: Vec<Vec<UseEdge>> = vec![Vec::new(); modules.len()];
    for (from, node) in modules.iter().enumerate() {
        for (use_index, use_target) in node.uses.iter().enumerate() {
            let Some(to) = resolved_module(modules, sources, settled, from, use_target) else {
                continue;
            };
            if to == from {
                continue;
            }
            if edges[from].iter().all(|edge| edge.to != to) {
                edges[from].push(UseEdge {
                    to,
                    use_index
                });
            }
        }
    }
    edges
}
