// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a capability rests on: the function that carries it and the written
//! call that proved it, located to the byte.
//!
//! A capability without this is an assertion. With it a reader opens the
//! file at the range and sees the same call the engine saw, which is the
//! only form of "trust me" this project accepts.

use rustmanifest_schema::Range;

use crate::semantic::{
    behavior::{EffectSite, FunctionFacts},
    module_overview,
    workspace::Workspace
};

/// The function and call a capability was read from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evidence {
    /// Qualified name of the function whose body carries the call.
    pub function: String,
    /// The written path that proved the capability.
    pub call:     String,
    /// Index of the source the call stands in.
    pub source:   usize,
    /// Byte range of the call within that source.
    pub range:    Range
}

impl Evidence {
    /// Records `site` inside `fact` as what proved a capability.
    pub(super) fn of(fact: &FunctionFacts, site: &EffectSite) -> Self {
        Self {
            function: fact.function.clone(),
            call:     site.proof.clone(),
            source:   fact.source,
            range:    site.range
        }
    }
}

/// Returns a lookup from source index to the module path written there.
///
/// A source no module claims answers `?`, the same notation the rest of the
/// tier uses for a name it could not resolve — the row is still stated,
/// with its ignorance visible.
pub(super) fn modules_by_source<'a>(
    workspace: &'a Workspace<'a>
) -> impl Fn(usize) -> String + use<'a> {
    let overview = module_overview::of(workspace.modules(), workspace.sources());
    let sources = workspace.sources();
    move |source: usize| {
        sources
            .get(source)
            .and_then(|entry| {
                overview
                    .modules()
                    .iter()
                    .find(|module| module.file == entry.path())
            })
            .map_or_else(|| "?".to_owned(), |module| module.path.clone())
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
    use crate::source::Source;

    #[test]
    fn a_source_inside_a_crate_answers_with_its_module_path() {
        let sources = vec![Source::new(
            PathBuf::from("app/src/lib.rs"),
            "pub fn go() {}".to_owned()
        )];
        let workspace = Workspace::new(&sources);
        assert_eq!(modules_by_source(&workspace)(0), "crate");
    }

    #[test]
    fn a_source_index_no_module_claims_answers_with_the_unresolved_mark() {
        let sources = vec![Source::new(
            PathBuf::from("app/src/lib.rs"),
            "pub fn go() {}".to_owned()
        )];
        let workspace = Workspace::new(&sources);
        assert_eq!(modules_by_source(&workspace)(7), "?");
    }
}
