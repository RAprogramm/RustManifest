// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Call chains a violation is derived from, and the provenance of the
//! edges they walk.
//!
//! A transitive violation is only as good as the path that proves it, so
//! the search returns the shortest resolved chain from the violating
//! function to the function that closes the case — the one performing the
//! effect, or the one sitting below the denied layer. Each hop of that
//! chain carries a reason naming how the edge was established.

use crate::semantic::{behavior::FunctionFacts, calls::CallGraph};

/// States how one traversed call edge was established, so a derivation
/// never claims more certainty than its weakest hop: dispatch over
/// several implementors admits possibility, an edge only the SCIP index
/// resolved names the index as its source, and everything else is the
/// syntactic resolver's unique match.
pub(in crate::semantic) fn edge_reason(graph: &CallGraph, from: usize, to: usize) -> &'static str {
    if graph.possibilistic(from, to) {
        "dispatch edge over several implementors — the effect is possible, not certain"
    } else if graph.indexed(from, to) {
        "name-resolved call edge, read from the SCIP index"
    } else {
        "resolved call edge"
    }
}

/// Finds the shortest resolved call chain from `start` to any function
/// matching `target`, both endpoints included.
pub(super) fn reach_chain(
    facts: &[FunctionFacts],
    graph: &CallGraph,
    start: usize,
    target: impl Fn(usize) -> bool
) -> Option<Vec<usize>> {
    let mut parent: Vec<Option<usize>> = vec![None; facts.len()];
    let mut visited = vec![false; facts.len()];
    let mut queue = std::collections::VecDeque::from([start]);
    visited[start] = true;
    while let Some(current) = queue.pop_front() {
        if current != start && target(current) {
            let mut chain = vec![current];
            let mut cursor = current;
            while let Some(previous) = parent[cursor] {
                chain.push(previous);
                cursor = previous;
            }
            chain.reverse();
            return Some(chain);
        }
        for &next in graph.callees(current) {
            if !visited[next] {
                visited[next] = true;
                parent[next] = Some(current);
                queue.push_back(next);
            }
        }
    }
    None
}

/// Finds the shortest resolved call chain from `start` to a function that
/// performs `effect` directly, both endpoints included; empty when no such
/// chain exists.
pub(in crate::semantic) fn effect_chain(
    facts: &[FunctionFacts],
    graph: &CallGraph,
    start: usize,
    effect: &str
) -> Vec<usize> {
    let mut parent: Vec<Option<usize>> = vec![None; facts.len()];
    let mut visited = vec![false; facts.len()];
    let mut queue = std::collections::VecDeque::from([start]);
    visited[start] = true;
    while let Some(current) = queue.pop_front() {
        if current != start
            && facts[current]
                .effects
                .iter()
                .any(|observed| observed.to_string() == effect)
        {
            let mut chain = vec![current];
            let mut cursor = current;
            while let Some(previous) = parent[cursor] {
                chain.push(previous);
                cursor = previous;
            }
            chain.reverse();
            return chain;
        }
        for &next in graph.callees(current) {
            if !visited[next] {
                visited[next] = true;
                parent[next] = Some(current);
                queue.push_back(next);
            }
        }
    }
    Vec::new()
}
