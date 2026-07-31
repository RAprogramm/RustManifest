// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! `module-cycles` check: cycles in the module dependency graph.
//!
//! The dependency graph is derived from the [`ModuleGraph`]: an edge from
//! module A to module B exists when a `use` target written in A resolves to
//! B within the same crate. `crate::`, `self::`, and `super::` heads are
//! rewritten into absolute paths against the source module; external and
//! unresolvable targets produce no edges, and so do self-imports. Each
//! cycle is reported once, rotated to start at its smallest module path.
//!
//! Two-module cycles between a module and its own ancestor are idiomatic in
//! Rust — a parent re-exports child items while the child names types from
//! the parent — so they are skipped.

use rustmanifest_schema::TraceStep;

use super::{
    Insight,
    module_graph::{ModuleGraph, ModuleNode}
};
use crate::source::Source;

/// Reports every module dependency cycle of the source slice, anchored at
/// the `use` edge leaving the first module of the canonical cycle.
pub(super) fn insights(sources: &[Source]) -> Vec<Insight> {
    let graph = ModuleGraph::from_sources(sources);
    let edges = dependency_edges(&graph);
    cycles(&graph, &edges)
        .iter()
        .filter_map(|cycle| cycle_insight(sources, &graph, &edges, cycle))
        .collect()
}

/// Enumerates every canonical module dependency cycle of the graph in
/// depth-first discovery order, skipping ancestor-descendant two-module
/// cycles and duplicates.
pub(super) fn cycles(graph: &ModuleGraph, edges: &[Vec<UseEdge>]) -> Vec<Vec<usize>> {
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

/// A resolved module-to-module edge with the use target it came from.
#[derive(Debug, Clone, Copy)]
pub(super) struct UseEdge {
    /// Index of the target module.
    pub(super) to:        usize,
    /// Index into the source module's `uses` list, kept as evidence.
    pub(super) use_index: usize
}

/// Builds per-module dependency edges from the expanded `use` targets.
///
/// Edges are deduplicated per module pair, keeping the first use target as
/// evidence, and self-edges are dropped.
pub(super) fn dependency_edges(graph: &ModuleGraph) -> Vec<Vec<UseEdge>> {
    let modules = graph.modules();
    let mut edges: Vec<Vec<UseEdge>> = vec![Vec::new(); modules.len()];
    for (from, node) in modules.iter().enumerate() {
        for (use_index, use_target) in node.uses.iter().enumerate() {
            let Some(to) = resolved_module(modules, from, &use_target.target) else {
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

/// Resolves `target`, as written inside the module at `from`, to a module
/// of the same crate.
///
/// The deepest module whose path covers the absolute target wins, so a
/// target naming an item resolves to the module defining it. External and
/// unresolvable targets yield `None`.
pub(super) fn resolved_module(modules: &[ModuleNode], from: usize, target: &str) -> Option<usize> {
    let source = modules.get(from)?;
    let absolute = absolute_target(&source.path, target)
        .or_else(|| uniform_path_target(modules, from, target))?;
    let mut best: Option<(usize, usize)> = None;
    for (index, module) in modules.iter().enumerate() {
        if module.krate != source.krate || !path_covers(&module.path, &absolute) {
            continue;
        }
        let depth = module.path.len();
        if best.is_none_or(|(_, best_depth)| depth > best_depth) {
            best = Some((index, depth));
        }
    }
    Some(best?.0)
}

/// Returns true when `prefix` equals `path` or is a `::`-aligned prefix of
/// it.
fn path_covers(prefix: &str, path: &str) -> bool {
    path == prefix || (path.starts_with(prefix) && path[prefix.len()..].starts_with("::"))
}

/// Rewrites `target` into an absolute `crate::` path relative to the module
/// path `source`, returning `None` for external or invalid targets.
fn absolute_target(source: &str, target: &str) -> Option<String> {
    let mut segments = target.split("::");
    let head = segments.next()?;
    match head {
        "crate" => Some(target.to_owned()),
        "self" => Some(joined(source, segments)),
        "super" => {
            let mut ancestors: Vec<&str> = source.split("::").collect();
            ancestors.pop();
            let mut tail: Vec<&str> = Vec::new();
            for segment in segments {
                if tail.is_empty() && segment == "super" {
                    ancestors.pop();
                } else {
                    tail.push(segment);
                }
            }
            if ancestors.is_empty() {
                return None;
            }
            ancestors.extend(tail);
            Some(ancestors.join("::"))
        }
        _ => None
    }
}

/// Resolves a Rust 2018 uniform path — a `use` target whose head names a
/// child module of the current module rather than a crate — into an
/// absolute path, or [`None`] when no such child module exists.
///
/// The child-module check keeps external crates out: `use serde::…` only
/// becomes an in-crate path if the current module really declares a
/// `serde` submodule.
fn uniform_path_target(modules: &[ModuleNode], from: usize, target: &str) -> Option<String> {
    let source = modules.get(from)?;
    let head = target.split("::").next()?;
    let child = format!("{}::{head}", source.path);
    modules
        .iter()
        .any(|module| module.krate == source.krate && module.path == child)
        .then(|| joined(&source.path, target.split("::")))
}

/// Appends `segments` to the module path `base` with `::` separators.
fn joined<'a>(base: &str, segments: impl Iterator<Item = &'a str>) -> String {
    let mut path = base.to_owned();
    for segment in segments {
        path.push_str("::");
        path.push_str(segment);
    }
    path
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

/// Builds the insight for one canonical cycle, anchoring at the edge
/// leaving the first module. The detail names only the cycle chain; every
/// `use` edge becomes one trace step.
fn cycle_insight(
    sources: &[Source],
    graph: &ModuleGraph,
    edges: &[Vec<UseEdge>],
    cycle: &[usize]
) -> Option<Insight> {
    let modules = graph.modules();
    let first = *cycle.first()?;
    let mut trace = Vec::with_capacity(cycle.len());
    for (offset, &from) in cycle.iter().enumerate() {
        let to = cycle[(offset + 1) % cycle.len()];
        let edge = edges.get(from)?.iter().find(|edge| edge.to == to)?;
        let use_target = modules[from].uses.get(edge.use_index)?;
        let file = sources.get(modules[from].source)?.path();
        trace.push(TraceStep {
            fact:   format!(
                "{} ({}) uses {}",
                modules[from].path,
                file.display(),
                use_target.target
            ),
            reason: format!("resolves to {}, an edge of the cycle", modules[to].path)
        });
    }
    let mut chain: Vec<&str> = cycle
        .iter()
        .map(|&module| modules[module].path.as_str())
        .collect();
    chain.push(modules[first].path.as_str());
    let detail = format!("module dependency cycle: {}", chain.join(" -> "));
    let anchor_edge = edges
        .get(first)?
        .iter()
        .find(|edge| edge.to == cycle[usize::from(cycle.len() > 1)])?;
    let range = modules[first].uses.get(anchor_edge.use_index)?.range;
    Some(Insight::new(modules[first].source, range, detail, trace))
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use std::path::PathBuf;

    use super::{absolute_target, insights, path_covers};
    use crate::source::Source;

    fn source(path: &str, text: &str) -> Source {
        Source::new(PathBuf::from(path), text.to_owned())
    }

    #[test]
    fn crate_targets_stay_absolute() {
        assert_eq!(
            absolute_target("crate::a", "crate::b::Item").as_deref(),
            Some("crate::b::Item")
        );
    }

    #[test]
    fn self_targets_extend_the_source_path() {
        assert_eq!(
            absolute_target("crate::a", "self::x").as_deref(),
            Some("crate::a::x")
        );
    }

    #[test]
    fn super_targets_pop_source_segments() {
        assert_eq!(
            absolute_target("crate::a::b", "super::c").as_deref(),
            Some("crate::a::c")
        );
        assert_eq!(
            absolute_target("crate::a::b", "super::super::c").as_deref(),
            Some("crate::c")
        );
    }

    #[test]
    fn super_above_the_crate_root_is_invalid() {
        assert_eq!(absolute_target("crate", "super::x"), None);
        assert_eq!(absolute_target("crate::a", "super::super::x"), None);
    }

    #[test]
    fn external_targets_are_not_resolved() {
        assert_eq!(absolute_target("crate::a", "std::fmt"), None);
        assert_eq!(absolute_target("crate::a", "serde::Serialize"), None);
    }

    #[test]
    fn coverage_is_segment_aligned() {
        assert!(path_covers("crate::a", "crate::a"));
        assert!(path_covers("crate::a", "crate::a::Item"));
        assert!(!path_covers("crate::a", "crate::ab"));
    }

    #[test]
    fn acyclic_uses_yield_no_insights() {
        let sources = [source(
            "src/lib.rs",
            "mod a { use crate::b::Item; } mod b { use std::fmt; pub struct Item; }"
        )];
        assert!(insights(&sources).is_empty());
    }

    #[test]
    fn inline_sibling_cycle_is_reported() {
        let sources = [source(
            "src/lib.rs",
            "mod a { use crate::b::Item; } mod b { use crate::a::Other; }"
        )];
        let found = insights(&sources);
        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0].detail,
            "module dependency cycle: crate::a -> crate::b -> crate::a"
        );
        assert!(!found[0].detail.contains("; "));
        assert_eq!(found[0].trace.len(), 2);
        assert_eq!(
            found[0].trace[0].fact,
            "crate::a (src/lib.rs) uses crate::b::Item"
        );
        assert_eq!(
            found[0].trace[0].reason,
            "resolves to crate::b, an edge of the cycle"
        );
        assert_eq!(
            found[0].trace[1].fact,
            "crate::b (src/lib.rs) uses crate::a::Other"
        );
    }

    #[test]
    fn cross_file_cycle_is_anchored_at_the_first_module_file() {
        let sources = [
            source("src/lib.rs", "mod a; mod b;"),
            source("src/a.rs", "use crate::b::Item;"),
            source("src/b.rs", "use crate::a::Other;")
        ];
        let found = insights(&sources);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].source, 1);
        assert_eq!(found[0].range.start, 0);
        assert_eq!(found[0].range.end, 19);
    }

    #[test]
    fn three_module_cycle_is_reported_once_with_canonical_rotation() {
        let sources = [source(
            "src/lib.rs",
            "mod b { use crate::c::X; } mod c { use crate::a::Y; } mod a { use crate::b::Z; }"
        )];
        let found = insights(&sources);
        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0].detail,
            "module dependency cycle: crate::a -> crate::b -> crate::c -> crate::a"
        );
        assert_eq!(found[0].trace.len(), 3);
    }

    #[test]
    fn parent_child_cycle_is_skipped() {
        let sources = [source(
            "src/lib.rs",
            "use crate::child::Item; mod child { use crate::Error; }"
        )];
        assert!(insights(&sources).is_empty());
    }

    #[test]
    fn longer_cycle_through_ancestor_stays_reported() {
        let sources = [source(
            "src/lib.rs",
            "use crate::a::X; mod a { use crate::b::Y; } mod b { use crate::Z; }"
        )];
        assert_eq!(insights(&sources).len(), 1);
    }

    #[test]
    fn self_import_is_not_a_cycle() {
        let sources = [source(
            "src/lib.rs",
            "mod a { use crate::a::Thing; use self::Widget; }"
        )];
        assert!(insights(&sources).is_empty());
    }

    #[test]
    fn external_and_unresolvable_targets_are_ignored() {
        let sources = [source(
            "src/lib.rs",
            "mod a { use std::fmt; use serde::Serialize; use crate::nope::Thing; }"
        )];
        assert!(insights(&sources).is_empty());
    }

    #[test]
    fn unreachable_file_is_invisible_to_the_check() {
        let sources = [
            source("src/lib.rs", "mod a;"),
            source("src/a.rs", "use std::fmt;"),
            source("src/orphan.rs", "use crate::a::Item;")
        ];
        assert!(insights(&sources).is_empty());
    }

    #[test]
    fn lone_fixture_file_falls_back_to_its_own_crate_root() {
        let sources = [source(
            "rules/fail.rs",
            "mod a { use crate::b::Item; } mod b { use crate::a::Other; }"
        )];
        assert_eq!(insights(&sources).len(), 1);
    }

    #[test]
    fn detection_is_deterministic() {
        let sources = [source(
            "src/lib.rs",
            "mod b { use crate::c::X; } mod c { use crate::a::Y; } mod a { use crate::b::Z; }"
        )];
        let first: Vec<String> = insights(&sources)
            .into_iter()
            .map(|insight| insight.detail)
            .collect();
        let second: Vec<String> = insights(&sources)
            .into_iter()
            .map(|insight| insight.detail)
            .collect();
        assert_eq!(first, second);
    }
}
