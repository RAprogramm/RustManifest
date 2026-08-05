// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The analyzed sources and everything the tier derives from them.
//!
//! A type graph, the behavioral facts of every function, a module graph, a
//! call graph — each is a property of the sources, not of the reader asking
//! about it. Deriving them inside each reader made a run's cost scale with
//! the number of readers: eight of the fourteen bundled checks read
//! behavioral facts, and a single `report` derived them six times over.
//! This module derives each one at most once per analysis and hands the
//! same value to every reader.
//!
//! Each derivation stays lazy, so a run that reads one of them never pays
//! for the rest.
//!
//! The derivations split by what they speak about: [`Declared`] holds what
//! the sources declare — types and modules — and [`Performed`] holds what
//! they do: per-function facts, the names a call resolves against, and the
//! call graph over them.

mod declared;
mod performed;

use self::{declared::Declared, performed::Performed};
use super::{
    behavior::FunctionFacts, calls::CallGraph, graph::TypeGraph, module_graph::ModuleGraph
};
use crate::source::Source;

/// The sources under analysis, with the workspace-wide derivations taken
/// from them.
///
/// The derivations hold `syn`-derived data, so a workspace stays on the
/// thread that built it.
#[derive(Debug)]
pub struct Workspace<'source> {
    /// Sources this analysis covers, in the order insights index into.
    sources:   &'source [Source],
    /// What the sources declare: the type graph and the module graph.
    declared:  Declared,
    /// What the sources do: function facts, name resolution, call graph.
    performed: Performed
}

impl<'source> Workspace<'source> {
    /// Takes `sources` as the material of one analysis. Nothing is derived
    /// until a reader asks for it.
    #[must_use]
    pub const fn new(sources: &'source [Source]) -> Self {
        Self {
            sources,
            declared: Declared::new(),
            performed: Performed::new()
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
        self.declared.types(self.sources)
    }

    /// Returns the behavioral facts of every function, deriving them on the
    /// first call.
    #[must_use]
    pub fn facts(&self) -> &[FunctionFacts] {
        self.performed.facts(self.sources)
    }

    /// Returns the module graph, deriving it on the first call.
    pub(in crate::semantic) fn modules(&self) -> &ModuleGraph {
        self.declared.modules(self.sources)
    }

    /// Returns the call graph over the workspace facts, deriving it — and
    /// the name resolution it needs — on the first call.
    pub(crate) fn calls(&self) -> &CallGraph {
        self.performed.calls(self.sources)
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

    /// Laziness is asserted against a freshly built workspace rather than
    /// an exact rendering, so the test states the property and not the
    /// wording the standard library happens to use.
    #[test]
    fn nothing_is_derived_before_the_first_read() {
        let sources = sources();
        let workspace = Workspace::new(&sources);
        assert_eq!(
            format!("{workspace:?}"),
            format!("{:?}", Workspace::new(&sources))
        );
        assert_eq!(workspace.types().types().len(), 1);
        assert_ne!(
            format!("{workspace:?}"),
            format!("{:?}", Workspace::new(&sources))
        );
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
        assert_eq!(workspace.facts().as_ptr(), workspace.facts().as_ptr());
        assert_eq!(workspace.facts().len(), 1);
    }

    #[test]
    fn the_analyzed_sources_are_handed_back() {
        let sources = sources();
        assert_eq!(Workspace::new(&sources).sources().len(), 1);
    }

    #[test]
    fn reading_one_derivation_leaves_the_others_underived() {
        let sources = sources();
        let types_only = Workspace::new(&sources);
        assert_eq!(types_only.types().types().len(), 1);
        let both = Workspace::new(&sources);
        assert_eq!(both.types().types().len(), 1);
        assert_eq!(both.facts().len(), 1);
        assert_ne!(format!("{types_only:?}"), format!("{both:?}"));
    }
}
