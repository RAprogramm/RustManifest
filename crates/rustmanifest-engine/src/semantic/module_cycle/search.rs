// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Cycle detection over the module dependency edges.
//!
//! A depth-first walk colours every module and records each cycle once,
//! rotated to start at its smallest module path. Two-module cycles between
//! a module and its own ancestor are idiomatic in Rust — a parent
//! re-exports child items while the child names types from the parent — so
//! they are skipped.

use super::edge::UseEdge;
use crate::semantic::module_graph::ModuleGraph;

/// Enumerates every canonical module dependency cycle of the graph in
/// depth-first discovery order, skipping ancestor-descendant two-module
/// cycles and duplicates.
pub(in crate::semantic) fn cycles(graph: &ModuleGraph, edges: &[Vec<UseEdge>]) -> Vec<Vec<usize>> {
    let mut found = Vec::new();
    let mut color = vec![Color::White; graph.modules().len()];
    for start in 0..graph.modules().len() {
        if color[start] == Color::White {
            depth_first(graph, edges, start, &mut color, &mut found);
        }
    }
    found
}

/// Traversal state of a module during depth-first search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Color {
    /// Not visited yet.
    White,
    /// On the active depth-first path.
    Gray,
    /// Fully explored.
    Black
}

/// Runs an iterative depth-first search from `start`, recording each
/// canonical cycle not yet present in `found`. Ancestor-descendant
/// two-module cycles are skipped.
fn depth_first(
    graph: &ModuleGraph,
    edges: &[Vec<UseEdge>],
    start: usize,
    color: &mut [Color],
    found: &mut Vec<Vec<usize>>
) {
    let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
    let mut path: Vec<usize> = vec![start];
    color[start] = Color::Gray;
    while let Some(frame) = stack.last_mut() {
        let (node, cursor) = *frame;
        if cursor == edges[node].len() {
            color[node] = Color::Black;
            stack.pop();
            path.pop();
            continue;
        }
        frame.1 += 1;
        let next = edges[node][cursor].to;
        match color[next] {
            Color::White => {
                color[next] = Color::Gray;
                stack.push((next, 0));
                path.push(next);
            }
            Color::Gray => {
                let Some(position) = path.iter().position(|&module| module == next) else {
                    continue;
                };
                let cycle = canonical_cycle(graph, &path[position..]);
                if !ancestry_pair(graph, &cycle) && !found.contains(&cycle) {
                    found.push(cycle);
                }
            }
            Color::Black => {}
        }
    }
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
