// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The analyzed sources and everything the tier derives from them.
//!
//! A type graph, the behavioral facts of every function, a module graph —
//! each is a property of the sources, not of the rule asking about it.
//! Deriving them inside each check made a run's cost scale with the size of
//! the rule set: eight of the fourteen bundled checks read behavioral facts,
//! so the workspace was walked for facts eight times over. This module
//! derives each one at most once per analysis and hands the same value to
//! every reader.
//!
//! Each derivation stays lazy, so a run of one cheap rule never pays for a
//! derivation nobody reads.

use std::cell::OnceCell;

use super::{
    behavior::{FunctionFacts, behavior},
    graph::TypeGraph,
    module_graph::ModuleGraph
};
use crate::source::Source;

/// The sources under semantic analysis, with the workspace-wide derivations
/// taken from them.
///
/// Held by reference to the sources and by value for each derivation; the
/// derivations hold `syn`-derived data, so a workspace stays on the thread
/// that built it.
pub struct Workspace<'source> {
    /// Sources this analysis covers, in the order insights index into.
    sources: &'source [Source],
    /// Structures, enums, traits and impl blocks across the sources.
    types:   OnceCell<TypeGraph>,
    /// Behavioral facts of every function across the sources.
    facts:   OnceCell<Vec<FunctionFacts>>,
    /// Crate roots, module tree and per-module `use` targets.
    modules: OnceCell<ModuleGraph>
}

impl std::fmt::Debug for Workspace<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Workspace")
            .field("sources", &self.sources.len())
            .field("types", &self.types.get().is_some())
            .field("facts", &self.facts.get().map(Vec::len))
            .field("modules", &self.modules.get().is_some())
            .finish()
    }
}

impl<'source> Workspace<'source> {
    /// Takes `sources` as the material of one analysis. Nothing is derived
    /// until a reader asks for it.
    #[must_use]
    pub const fn new(sources: &'source [Source]) -> Self {
        Self {
            sources,
            types: OnceCell::new(),
            facts: OnceCell::new(),
            modules: OnceCell::new()
        }
    }

    /// Returns the analyzed sources.
    #[must_use]
    pub const fn sources(&self) -> &'source [Source] {
        self.sources
    }

    /// Returns the workspace type graph, deriving it on the first call.
    #[must_use]
    pub fn types(&self) -> &TypeGraph {
        self.types
            .get_or_init(|| TypeGraph::from_sources(self.sources))
    }

    /// Returns the behavioral facts of every function, deriving them on the
    /// first call.
    #[must_use]
    pub fn facts(&self) -> &[FunctionFacts] {
        self.facts.get_or_init(|| behavior(self.sources))
    }

    /// Returns the module graph, deriving it on the first call.
    pub(super) fn modules(&self) -> &ModuleGraph {
        self.modules
            .get_or_init(|| ModuleGraph::from_sources(self.sources))
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

    use super::*;

    fn sources() -> Vec<Source> {
        vec![Source::new(
            PathBuf::from("src/lib.rs"),
            "struct User { id: u64 }\nfn read_id(user: &User) -> u64 { user.id }\n".to_owned()
        )]
    }

    #[test]
    fn nothing_is_derived_before_the_first_read() {
        let sources = sources();
        let workspace = Workspace::new(&sources);
        let rendered = format!("{workspace:?}");
        assert!(rendered.contains("types: false"));
        assert!(rendered.contains("facts: None"));
        assert!(rendered.contains("modules: false"));
    }

    #[test]
    fn repeated_reads_share_one_type_graph() {
        let sources = sources();
        let workspace = Workspace::new(&sources);
        let first = std::ptr::from_ref(workspace.types());
        let second = std::ptr::from_ref(workspace.types());
        assert_eq!(first, second);
        assert_eq!(workspace.types().types().len(), 1);
    }

    #[test]
    fn repeated_reads_share_one_fact_set() {
        let sources = sources();
        let workspace = Workspace::new(&sources);
        let first = workspace.facts().as_ptr();
        let second = workspace.facts().as_ptr();
        assert_eq!(first, second);
        assert_eq!(workspace.facts().len(), 1);
    }

    #[test]
    fn the_analyzed_sources_are_handed_back() {
        let sources = sources();
        let workspace = Workspace::new(&sources);
        assert_eq!(workspace.sources().len(), 1);
    }

    #[test]
    fn reading_one_derivation_leaves_the_others_underived() {
        let sources = sources();
        let workspace = Workspace::new(&sources);
        assert_eq!(workspace.types().types().len(), 1);
        let rendered = format!("{workspace:?}");
        assert!(rendered.contains("types: true"));
        assert!(rendered.contains("facts: None"));
    }
}
