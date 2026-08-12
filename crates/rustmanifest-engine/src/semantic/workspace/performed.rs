// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What the analyzed sources do: per-function facts, the names their calls
//! resolve against, and the call graph built over both.
//!
//! Extracting the facts is the single most expensive derivation the engine
//! makes, and nearly every check and every report section reads them, so
//! they are extracted once per analysis and shared. The call graph sits
//! beside them because it is derived from them and from nothing else the
//! caller supplies.

use std::cell::OnceCell;

use crate::{
    semantic::{
        behavior::FunctionFacts,
        calls::{CallGraph, ResolutionContext, call_graph, resolution_context_with}
    },
    source::Source
};

/// The behavior-level derivations of one analysis, each taken on first
/// demand.
#[derive(Debug, Default)]
pub(super) struct Performed {
    /// Behavioral facts of every function across the sources.
    facts:   OnceCell<Vec<FunctionFacts>>,
    /// Names a call in these sources can resolve against.
    context: OnceCell<ResolutionContext>,
    /// Call graph over [`Self::facts`] resolved through [`Self::context`].
    calls:   OnceCell<CallGraph>
}

impl Performed {
    /// Creates an empty set; nothing is derived until a reader asks.
    pub(super) const fn new() -> Self {
        Self {
            facts:   OnceCell::new(),
            context: OnceCell::new(),
            calls:   OnceCell::new()
        }
    }

    /// Returns the facts of `sources` read with what a resolver settled
    /// about them, deriving them on the first call.
    pub(super) fn resolved_facts(
        &self,
        sources: &[Source],
        resolution: Option<&rustmanifest_schema::Resolution>
    ) -> &[FunctionFacts] {
        self.facts
            .get_or_init(|| crate::semantic::behavior::behavior_with(sources, resolution))
    }

    /// Returns the resolution context of `sources`, building it on the
    /// first call.
    pub(super) fn context(
        &self,
        sources: &[Source],
        resolution: Option<&rustmanifest_schema::Resolution>
    ) -> &ResolutionContext {
        self.context
            .get_or_init(|| resolution_context_with(sources, resolution))
    }

    /// Returns the call graph of `sources`, building it — and whatever it
    /// needs — on the first call.
    pub(super) fn calls(
        &self,
        sources: &[Source],
        resolution: Option<&rustmanifest_schema::Resolution>
    ) -> &CallGraph {
        self.calls.get_or_init(|| {
            call_graph(
                self.resolved_facts(sources, resolution),
                self.context(sources, resolution)
            )
        })
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
            "fn inner() {}\nfn outer() { inner(); }\n".to_owned()
        )]
    }

    /// Laziness is asserted against a freshly built set rather than an
    /// exact rendering, so the test states the property and not the
    /// wording the standard library happens to use.
    #[test]
    fn nothing_is_derived_before_the_first_read() {
        let sources = sources();
        let performed = Performed::new();
        assert_eq!(format!("{performed:?}"), format!("{:?}", Performed::new()));
        assert_eq!(performed.resolved_facts(&sources, None).len(), 2);
        assert_ne!(format!("{performed:?}"), format!("{:?}", Performed::new()));
    }

    #[test]
    fn facts_are_extracted_once_and_reused() {
        let sources = sources();
        let performed = Performed::new();
        assert_eq!(
            performed.resolved_facts(&sources, None).as_ptr(),
            performed.resolved_facts(&sources, None).as_ptr()
        );
        assert_eq!(performed.resolved_facts(&sources, None).len(), 2);
    }

    #[test]
    fn a_call_graph_is_built_once_and_reused() {
        let sources = sources();
        let performed = Performed::new();
        let first = std::ptr::from_ref(performed.calls(&sources, None));
        let second = std::ptr::from_ref(performed.calls(&sources, None));
        assert_eq!(first, second);
    }

    #[test]
    fn the_call_graph_derives_the_facts_it_needs() {
        let sources = sources();
        let calls_first = Performed::new();
        let _ = calls_first.calls(&sources, None);
        let facts_then_calls = Performed::new();
        assert_eq!(facts_then_calls.resolved_facts(&sources, None).len(), 2);
        let _ = facts_then_calls.calls(&sources, None);
        assert_eq!(format!("{calls_first:?}"), format!("{facts_then_calls:?}"));
    }

    #[test]
    fn reading_facts_leaves_the_call_graph_unbuilt() {
        let sources = sources();
        let facts_only = Performed::new();
        assert_eq!(facts_only.resolved_facts(&sources, None).len(), 2);
        let with_calls = Performed::new();
        let _ = with_calls.calls(&sources, None);
        assert_ne!(format!("{facts_only:?}"), format!("{with_calls:?}"));
    }
}
