// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Cycle detection over the module dependency edges via strongly connected
//! components.
//!
//! Tarjan's algorithm on the module dependency graph yields strongly
//! connected components; each component with more than one module is a
//! cycle. Two-module cycles between a module and its own ancestor are
//! idiomatic in Rust — a parent re-exports child items while the child
//! names types from the parent — so they are skipped. Each cycle is
//! reported once, rotated to start at its smallest module path.

use petgraph::{Directed, Graph, algo::tarjan_scc};

use super::edge::UseEdge;
use crate::semantic::module_graph::ModuleGraph;

/// Enumerates every canonical module dependency cycle of the graph via
/// strongly connected components, skipping ancestor-descendant two-module
/// cycles.
pub(in crate::semantic) fn cycles(graph: &ModuleGraph, edges: &[Vec<UseEdge>]) -> Vec<Vec<usize>> {
    let module_count = graph.modules().len();
    let mut g: Graph<usize, (), Directed> = Graph::with_capacity(module_count, 0);
    let nodes: Vec<_> = (0..module_count).map(|module| g.add_node(module)).collect();

    for (from, from_edges) in edges.iter().enumerate() {
        for edge in from_edges {
            g.add_edge(nodes[from], nodes[edge.to], ());
        }
    }

    tarjan_scc(&g)
        .into_iter()
        .filter_map(|scc| {
            if scc.len() <= 1 {
                return None;
            }
            let cycle: Vec<usize> = scc.iter().map(|node| g[*node]).collect();
            let canonical = canonical_cycle(graph, &cycle);
            if ancestry_pair(graph, &canonical) {
                None
            } else {
                Some(canonical)
            }
        })
        .collect()
}

/// Returns true when `cycle` is a two-module cycle between a module and one
/// of its ancestors.
fn ancestry_pair(graph: &ModuleGraph, cycle: &[usize]) -> bool {
    if cycle.len() != 2 {
        return false;
    }
    let first = &graph.modules()[cycle[0]].path;
    let second = &graph.modules()[cycle[1]].path;
    descends_from(first, second) || descends_from(second, first)
}

/// Returns true when `child` is nested somewhere below `ancestor`.
fn descends_from(child: &str, ancestor: &str) -> bool {
    child
        .strip_prefix(ancestor)
        .is_some_and(|rest| rest.starts_with("::"))
}

/// Rotates `cycle` to start at the module with the smallest path.
fn canonical_cycle(graph: &ModuleGraph, cycle: &[usize]) -> Vec<usize> {
    let pivot = (0..cycle.len())
        .min_by_key(|&offset| &graph.modules()[cycle[offset]].path)
        .unwrap_or(0);
    let mut rotated = Vec::with_capacity(cycle.len());
    rotated.extend_from_slice(&cycle[pivot..]);
    rotated.extend_from_slice(&cycle[..pivot]);
    rotated
}
