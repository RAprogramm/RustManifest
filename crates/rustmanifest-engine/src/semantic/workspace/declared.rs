// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What the analyzed sources declare: their types and their modules.
//!
//! Both graphs are functions of the sources alone, and both are read by
//! several checks and reports at once, so each is built at most once per
//! analysis and shared afterwards.

use std::cell::OnceCell;

use crate::{
    semantic::{graph::TypeGraph, module_graph::ModuleGraph},
    source::Source
};

/// The declaration-level graphs of one analysis, each built on first
/// demand.
#[derive(Debug, Default)]
pub(super) struct Declared {
    /// Structures, enums, traits and impl blocks across the sources.
    types:   OnceCell<TypeGraph>,
    /// Crate roots, module tree and per-module `use` targets.
    modules: OnceCell<ModuleGraph>
}

impl Declared {
    /// Creates an empty set; nothing is built until a reader asks.
    pub(super) const fn new() -> Self {
        Self {
            types:   OnceCell::new(),
            modules: OnceCell::new()
        }
    }

    /// Returns the type graph of `sources`, building it on the first call.
    pub(super) fn types(
        &self,
        sources: &[Source],
        settled: Option<&rustmanifest_schema::Resolution>
    ) -> &TypeGraph {
        self.types.get_or_init(|| {
            let mut graph = TypeGraph::from_sources(sources);
            graph.granted_by(settled);
            graph
        })
    }

    /// Returns the module graph of `sources`, building it on the first
    /// call.
    pub(super) fn modules(
        &self,
        sources: &[Source],
        settled: Option<&rustmanifest_schema::Resolution>
    ) -> &ModuleGraph {
        self.modules
            .get_or_init(|| ModuleGraph::of(sources, settled))
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
            "struct User { id: u64 }".to_owned()
        )]
    }

    /// Laziness is asserted against a freshly built set rather than an
    /// exact rendering, so the test states the property and not the
    /// wording the standard library happens to use.
    #[test]
    fn nothing_is_built_before_the_first_read() {
        let sources = sources();
        let declared = Declared::new();
        assert_eq!(format!("{declared:?}"), format!("{:?}", Declared::new()));
        assert_eq!(declared.types(&sources, None).types().len(), 1);
        assert_ne!(format!("{declared:?}"), format!("{:?}", Declared::new()));
    }

    #[test]
    fn a_type_graph_is_built_once_and_reused() {
        let sources = sources();
        let declared = Declared::new();
        let first = std::ptr::from_ref(declared.types(&sources, None));
        let second = std::ptr::from_ref(declared.types(&sources, None));
        assert_eq!(first, second);
        assert_eq!(declared.types(&sources, None).types().len(), 1);
    }

    #[test]
    fn a_module_graph_is_built_once_and_reused() {
        let sources = sources();
        let declared = Declared::new();
        let first = std::ptr::from_ref(declared.modules(&sources, None));
        let second = std::ptr::from_ref(declared.modules(&sources, None));
        assert_eq!(first, second);
    }

    #[test]
    fn reading_one_graph_leaves_the_other_unbuilt() {
        let sources = sources();
        let types_only = Declared::new();
        assert_eq!(types_only.types(&sources, None).types().len(), 1);
        let both = Declared::new();
        assert_eq!(both.types(&sources, None).types().len(), 1);
        let _ = both.modules(&sources, None);
        assert_ne!(format!("{types_only:?}"), format!("{both:?}"));
    }
}
