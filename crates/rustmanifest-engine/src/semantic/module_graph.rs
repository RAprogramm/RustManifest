// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Module graph of the analyzed workspace slice: crate roots, `mod`
//! resolution, module paths, and expanded `use` targets.
//!
//! The graph is built purely from the [`Source`] slice, never from the
//! filesystem: a `mod name;` declaration resolves to the source whose path
//! is `name.rs` or `name/mod.rs` relative to the declaring file's
//! directory, and inline `mod name { .. }` blocks recurse in place. Every
//! reachable module receives a path of the form `crate`, `crate::a`,
//! `crate::a::b`.
//!
//! Crate roots follow cargo's target layout: `src/lib.rs`, `src/main.rs`,
//! `src/bin/*`, `build.rs`, and the top level of `tests/`, `benches/`,
//! and `examples/` (flat files and `*/main.rs`). Every source still
//! unclaimed after those roots are walked — compile-test fixtures, files
//! with broken `mod` chains, lone reviewed files — becomes its own
//! standalone root, marked `layout: false`, so no parsed file is invisible
//! to module-level checks. A standalone file extracted before a later
//! root claims it can appear under both crates; counters that deduplicate
//! by source are unaffected.

mod layout;
mod resolve;
mod tree;

use std::path::{Path, PathBuf};

use rustmanifest_schema::Range;

pub(super) use self::resolve::resolved_module;
use self::{layout::is_crate_root, tree::GraphBuilder};
use crate::source::Source;

/// One expanded `use` target recorded in a module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct UseTarget {
    /// Flat `::`-joined path as written, without a leading `::`.
    pub(super) target: String,
    /// Byte range of the `use` item that declared the target.
    pub(super) range:  Range
}

/// One module reachable from a crate root of the source slice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ModuleNode {
    /// Index of the crate root the module belongs to.
    pub(super) krate:  usize,
    /// Module path, `crate` for a crate root.
    pub(super) path:   String,
    /// Index of the source the module's items are written in.
    pub(super) source: usize,
    /// Whether the crate root follows cargo's target layout; `false` for
    /// standalone fallback roots.
    pub(super) layout: bool,
    /// Expanded `use` targets in declaration order.
    pub(super) uses:   Vec<UseTarget>
}

/// All modules reachable from the crate roots of the source slice.
#[derive(Debug, Default)]
pub(super) struct ModuleGraph {
    modules: Vec<ModuleNode>
}

impl ModuleGraph {
    /// Builds the graph by walking every layout crate root of `sources`,
    /// then promoting each still-unclaimed source to a standalone root;
    /// files that do not parse as Rust contribute no modules.
    pub(super) fn from_sources(sources: &[Source]) -> Self {
        let roots: Vec<usize> = (0..sources.len())
            .filter(|&index| is_crate_root(sources[index].path()))
            .collect();
        let mut builder = GraphBuilder {
            sources,
            by_path: sources
                .iter()
                .enumerate()
                .map(|(index, source)| (source.path(), index))
                .collect(),
            modules: Vec::new(),
            layout: true
        };
        let mut krate = 0;
        for &root in &roots {
            if builder.claimed(root) {
                continue;
            }
            let dir = sources[root]
                .path()
                .parent()
                .map_or_else(PathBuf::new, Path::to_path_buf);
            builder.extract_file(krate, "crate".to_owned(), root, dir);
            krate += 1;
        }
        builder.layout = false;
        for (index, source) in sources.iter().enumerate() {
            if builder.claimed(index) {
                continue;
            }
            let dir = source
                .path()
                .parent()
                .map_or_else(PathBuf::new, Path::to_path_buf);
            builder.extract_file(krate, "crate".to_owned(), index, dir);
            krate += 1;
        }
        Self {
            modules: builder.modules
        }
    }

    /// Modules in deterministic root-then-declaration order.
    pub(super) fn modules(&self) -> &[ModuleNode] {
        &self.modules
    }
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

    use super::ModuleGraph;
    use crate::source::Source;

    fn source(path: &str, text: &str) -> Source {
        Source::new(PathBuf::from(path), text.to_owned())
    }

    fn paths(graph: &ModuleGraph) -> Vec<String> {
        graph
            .modules()
            .iter()
            .map(|module| module.path.clone())
            .collect()
    }

    #[test]
    fn mod_declaration_resolves_flat_file() {
        let sources = [
            source("src/lib.rs", "mod util;"),
            source("src/util.rs", "use crate::Item;")
        ];
        let graph = ModuleGraph::from_sources(&sources);
        assert_eq!(paths(&graph), vec!["crate", "crate::util"]);
        assert_eq!(graph.modules()[1].source, 1);
        assert_eq!(graph.modules()[1].uses.len(), 1);
        assert_eq!(graph.modules()[1].uses[0].target, "crate::Item");
    }

    #[test]
    fn mod_declaration_resolves_directory_module() {
        let sources = [
            source("src/lib.rs", "mod nested;"),
            source("src/nested/mod.rs", "mod deep;"),
            source("src/nested/deep.rs", "")
        ];
        let graph = ModuleGraph::from_sources(&sources);
        assert_eq!(
            paths(&graph),
            vec!["crate", "crate::nested", "crate::nested::deep"]
        );
    }

    #[test]
    fn inline_modules_recurse() {
        let sources = [source(
            "src/lib.rs",
            "mod outer { mod inner { use std::fmt; } }"
        )];
        let graph = ModuleGraph::from_sources(&sources);
        assert_eq!(
            paths(&graph),
            vec!["crate", "crate::outer", "crate::outer::inner"]
        );
        assert_eq!(graph.modules()[2].uses[0].target, "std::fmt");
    }

    #[test]
    fn unreachable_file_becomes_a_standalone_root() {
        let sources = [
            source("src/lib.rs", "mod util;"),
            source("src/util.rs", ""),
            source("src/orphan.rs", "use crate::util::Item;")
        ];
        let graph = ModuleGraph::from_sources(&sources);
        assert_eq!(paths(&graph), vec!["crate", "crate::util", "crate"]);
        assert!(graph.modules()[0].layout);
        assert!(graph.modules()[1].layout);
        assert!(!graph.modules()[2].layout);
        assert_eq!(graph.modules()[2].krate, 1);
    }

    #[test]
    fn cargo_target_layout_files_are_crate_roots() {
        let sources = [
            source("src/lib.rs", ""),
            source("src/bin/tool.rs", ""),
            source("src/bin/other/main.rs", ""),
            source("build.rs", ""),
            source("tests/integration.rs", "mod common;"),
            source("tests/common.rs", ""),
            source("tests/suite/main.rs", ""),
            source("benches/throughput.rs", ""),
            source("examples/demo.rs", "")
        ];
        let graph = ModuleGraph::from_sources(&sources);
        assert!(graph.modules().iter().all(|module| module.layout));
        assert_eq!(
            paths(&graph),
            vec![
                "crate",
                "crate",
                "crate",
                "crate",
                "crate",
                "crate::common",
                "crate",
                "crate",
                "crate"
            ]
        );
    }

    #[test]
    fn every_file_is_a_root_when_no_crate_root_exists() {
        let sources = [
            source("rules/fixture.rs", "mod a { use std::fmt; }"),
            source("rules/other.rs", "")
        ];
        let graph = ModuleGraph::from_sources(&sources);
        assert_eq!(paths(&graph), vec!["crate", "crate::a", "crate"]);
        assert_eq!(graph.modules()[0].krate, 0);
        assert_eq!(graph.modules()[2].krate, 1);
    }

    #[test]
    fn main_rs_is_a_crate_root() {
        let sources = [source("src/main.rs", "mod cli;"), source("src/cli.rs", "")];
        let graph = ModuleGraph::from_sources(&sources);
        assert_eq!(paths(&graph), vec!["crate", "crate::cli"]);
    }

    #[test]
    fn unparsable_file_contributes_no_modules() {
        let sources = [
            source("src/lib.rs", "mod broken;"),
            source("src/broken.rs", "fn broken(")
        ];
        let graph = ModuleGraph::from_sources(&sources);
        assert_eq!(paths(&graph), vec!["crate"]);
    }

    #[test]
    fn use_ranges_span_the_declaring_item() {
        let sources = [source("src/lib.rs", "use std::fmt;")];
        let graph = ModuleGraph::from_sources(&sources);
        let target = &graph.modules()[0].uses[0];
        assert_eq!(target.range.start, 0);
        assert_eq!(target.range.end, 13);
    }

    #[test]
    fn graph_construction_is_deterministic() {
        let sources = [
            source("src/lib.rs", "mod a; mod b { use crate::a::Item; }"),
            source("src/a.rs", "use std::fmt;")
        ];
        let first = ModuleGraph::from_sources(&sources);
        let second = ModuleGraph::from_sources(&sources);
        assert_eq!(first.modules(), second.modules());
    }
}
