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
//!
//! The module splits along responsibilities: [`edge`] derives the
//! dependency edges from `use` targets, [`search`] finds the canonical
//! cycles over those edges, and [`report`] turns one cycle into the
//! insight it is reported as. This file holds only the drive between them.

mod edge;
mod report;
mod search;

use self::report::cycle_insight;
pub(super) use self::{edge::dependency_edges, search::cycles};
use super::{insight::Insight, module_graph::ModuleGraph};
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

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use std::path::PathBuf;

    use super::insights;
    use crate::source::Source;

    fn source(path: &str, text: &str) -> Source {
        Source::new(PathBuf::from(path), text.to_owned())
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
