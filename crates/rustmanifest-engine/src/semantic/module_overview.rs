// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Read-only view of the module graph for external consumers.
//!
//! [`module_overview`] condenses the engine-internal module graph into a
//! plain summary: every module reachable from a crate root, the resolved
//! intra-crate dependency edges between them, and whether a module
//! participates in a dependency cycle. Diagram exporters and other tooling
//! consume this view without touching the graph machinery itself.

use std::{collections::HashSet, path::PathBuf};

use super::{module_cycle, module_graph::ModuleGraph};
use crate::{semantic::workspace::Workspace, source::Source};

/// One module of the overview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleInfo {
    /// Module path, `crate` for a crate root.
    pub path:       String,
    /// Path of the source file the module's items are written in.
    pub file:       PathBuf,
    /// True when the module is the root module of its crate.
    pub crate_root: bool,
    /// True when the module participates in a module dependency cycle.
    pub in_cycle:   bool
}

/// One resolved intra-crate dependency between two modules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleEdge {
    /// Path of the module the dependency is written in.
    pub from: String,
    /// Path of the module the dependency resolves to.
    pub to:   String
}

/// Modules and dependency edges of the analyzed source slice.
#[derive(Debug, Default)]
pub struct ModuleOverview {
    modules: Vec<ModuleInfo>,
    edges:   Vec<ModuleEdge>
}

impl ModuleOverview {
    /// Modules in deterministic root-then-declaration order.
    #[must_use]
    pub fn modules(&self) -> &[ModuleInfo] {
        &self.modules
    }

    /// Dependency edges in deterministic per-module declaration order.
    #[must_use]
    pub fn edges(&self) -> &[ModuleEdge] {
        &self.edges
    }
}

/// Builds the module overview of `sources`.
///
/// The overview is derived from the same module graph and `use` resolution
/// the `module-cycles` check runs on: an edge from module A to module B
/// exists when a `use` target written in A resolves to B within the same
/// crate, and cycle membership follows the check's cycle detection,
/// including its exemption for ancestor-descendant two-module cycles.
#[must_use]
pub fn module_overview(workspace: &Workspace<'_>) -> ModuleOverview {
    of(
        workspace.modules(),
        workspace.sources(),
        workspace.resolution()
    )
}

/// Builds the overview from an already-built module graph.
///
/// Name resolution builds its own graph here rather than reading one off a
/// workspace: resolution is itself one of the workspace's derivations, and
/// asking the workspace for it from inside would re-enter it.
pub(in crate::semantic) fn of(
    graph: &ModuleGraph,
    sources: &[Source],
    settled: Option<&rustmanifest_schema::Resolution>
) -> ModuleOverview {
    let edges = module_cycle::dependency_edges(graph, sources, settled);
    let cyclic: HashSet<usize> = module_cycle::cycles(graph, &edges)
        .into_iter()
        .flatten()
        .collect();
    let modules = graph
        .modules()
        .iter()
        .enumerate()
        .map(|(index, module)| ModuleInfo {
            path:       module.path.clone(),
            file:       sources
                .get(module.source)
                .map(|source| source.path().to_path_buf())
                .unwrap_or_default(),
            crate_root: module.path == "crate",
            in_cycle:   cyclic.contains(&index)
        })
        .collect();
    let modules_by_index = graph.modules();
    let edge_list = edges
        .iter()
        .enumerate()
        .flat_map(|(from, targets)| {
            targets.iter().map(move |edge| ModuleEdge {
                from: modules_by_index[from].path.clone(),
                to:   modules_by_index[edge.to].path.clone()
            })
        })
        .collect();
    ModuleOverview {
        modules,
        edges: edge_list
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

    use super::module_overview;
    use crate::{
        semantic::{module_graph::declared_tree, workspace::Workspace},
        source::Source
    };

    fn source(path: &str, text: &str) -> Source {
        Source::new(PathBuf::from(path), text.to_owned())
    }

    #[test]
    fn modules_and_edges_are_reported() {
        let sources = [
            source("src/lib.rs", "mod a; mod b;"),
            source("src/a.rs", "use crate::b::Item;"),
            source("src/b.rs", "")
        ];
        let overview = module_overview(&Workspace::resolved(&sources, &declared_tree(&sources)));
        let paths: Vec<&str> = overview
            .modules()
            .iter()
            .map(|module| module.path.as_str())
            .collect();
        assert_eq!(paths, vec!["crate", "crate::a", "crate::b"]);
        assert!(overview.modules()[0].crate_root);
        assert!(!overview.modules()[1].crate_root);
        assert_eq!(overview.modules()[1].file, PathBuf::from("src/a.rs"));
        assert_eq!(overview.edges().len(), 1);
        assert_eq!(overview.edges()[0].from, "crate::a");
        assert_eq!(overview.edges()[0].to, "crate::b");
    }

    #[test]
    fn cycle_members_are_flagged() {
        let sources = [source(
            "src/lib.rs",
            "mod a { use crate::b::Item; } mod b { use crate::a::Other; }"
        )];
        let overview = module_overview(&Workspace::resolved(&sources, &declared_tree(&sources)));
        let flagged: Vec<(&str, bool)> = overview
            .modules()
            .iter()
            .map(|module| (module.path.as_str(), module.in_cycle))
            .collect();
        assert_eq!(
            flagged,
            vec![("crate", false), ("crate::a", true), ("crate::b", true)]
        );
    }

    #[test]
    fn parent_child_cycle_is_not_flagged() {
        let sources = [source(
            "src/lib.rs",
            "use crate::child::Item; mod child { use crate::Error; }"
        )];
        let overview = module_overview(&Workspace::resolved(&sources, &declared_tree(&sources)));
        assert!(overview.modules().iter().all(|module| !module.in_cycle));
        assert_eq!(overview.edges().len(), 2);
    }

    #[test]
    fn overview_is_deterministic() {
        let sources = [
            source("src/lib.rs", "mod a; mod b;"),
            source("src/a.rs", "use crate::b::Item;"),
            source("src/b.rs", "use crate::a::Other;")
        ];
        let first = module_overview(&Workspace::resolved(&sources, &declared_tree(&sources)));
        let second = module_overview(&Workspace::resolved(&sources, &declared_tree(&sources)));
        assert_eq!(first.modules(), second.modules());
        assert_eq!(first.edges(), second.edges());
    }
}
