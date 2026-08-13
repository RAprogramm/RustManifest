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
        .filter_map(|component| {
            if component.len() <= 1 {
                return None;
            }
            let members: Vec<usize> = component.iter().map(|node| g[*node]).collect();
            let ordered = walked(graph, edges, &members)?;
            (!ancestry_pair(graph, &ordered)).then_some(ordered)
        })
        .collect()
}

/// Walks one strongly connected component back into the cycle it stands
/// for, starting at the module with the smallest path.
///
/// A component is a set of modules that all reach each other; the cycle a
/// reader is shown is a path through them, and which module follows which
/// is written in the edges, not in the set.
fn walked(graph: &ModuleGraph, edges: &[Vec<UseEdge>], members: &[usize]) -> Option<Vec<usize>> {
    let start = *members
        .iter()
        .min_by_key(|module| &graph.modules()[**module].path)?;
    let mut ordered = vec![start];
    loop {
        let standing = *ordered.last()?;
        let next = edges
            .get(standing)?
            .iter()
            .map(|edge| edge.to)
            .filter(|reached| members.contains(reached))
            .find(|reached| !ordered.contains(reached));
        match next {
            Some(reached) => ordered.push(reached),
            None => break
        }
    }
    (ordered.len() == members.len()).then_some(ordered)
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
