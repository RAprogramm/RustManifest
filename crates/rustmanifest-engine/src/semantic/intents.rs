// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Intents derived from written evidence: the program reads its own
//! workspace and names what is missing, before anyone asks.
//!
//! The evidence is a written call that only an installed ability can
//! satisfy: a call to a workspace type's `default` where the type
//! carries no `Default`, or to its `to_string` — the blanket the
//! standard library grants over `Display` — where it carries no
//! `Display`. Such a call is the author's stated intent in the
//! program's own language, and the derivation cites the function that
//! wrote it. Each intent then runs the full deduction: per-field
//! proofs, the catalog's choice, the montage, and the re-reading —
//! with the evidence standing as the chain's first step, so the
//! finished chain reads from the written need to the confirmed gain.

use rustmanifest_schema::{Range, TraceStep};

use super::{
    behavior::behavior,
    deduce::{Deduction, deduce_impl},
    graph::TypeGraph
};
use crate::source::Source;

/// One intent the evidence proves, with the written call that proves
/// it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedIntent {
    /// The type the intent is about.
    pub type_name:  String,
    /// The ability the evidence asks for.
    pub trait_name: String,
    /// The function whose written call states the intent.
    pub asked_by:   String,
    /// The written call itself, as the body spells it.
    pub call:       String,
    /// Index of the source whose body writes the call.
    pub source:     usize,
    /// Byte range of the calling construct — the evidence's written
    /// address, so a surface can point at the site itself.
    pub site:       Range,
    /// The written call standing as evidence, spelled as a chain step.
    pub evidence:   TraceStep
}

/// The method names whose written call states an intent, with the
/// ability each one asks for.
const ASKING_CALLS: [(&str, &str); 2] = [("default", "Default"), ("to_string", "Display")];

/// Reads the workspace and derives every intent its written calls
/// prove, deduplicated by type and ability.
#[must_use]
pub fn derived_intents(sources: &[Source]) -> Vec<DerivedIntent> {
    derived_intents_with(&behavior(sources), &TypeGraph::from_sources(sources))
}

/// Derives the intents over facts and a type graph already computed,
/// so a caching caller pays for neither twice.
#[must_use]
pub fn derived_intents_with(
    facts: &[super::behavior::FunctionFacts],
    types: &TypeGraph
) -> Vec<DerivedIntent> {
    let mut found: Vec<DerivedIntent> = Vec::new();
    for fact in facts {
        for place in &fact.call_sites {
            let Some((owner, method)) = place.candidate.split_once("::") else {
                continue;
            };
            let Some((_, trait_name)) = ASKING_CALLS.iter().find(|(asking, _)| *asking == method)
            else {
                continue;
            };
            if types.type_named(owner).is_none() {
                continue;
            }
            if types.ability_given(owner, trait_name) {
                continue;
            }
            let already = found
                .iter()
                .any(|held| held.type_name == owner && held.trait_name == *trait_name);
            if already {
                continue;
            }
            found.push(DerivedIntent {
                type_name:  owner.to_owned(),
                trait_name: (*trait_name).to_owned(),
                asked_by:   fact.function.clone(),
                call:       place.candidate.clone(),
                source:     fact.source,
                site:       place.site,
                evidence:   TraceStep {
                    fact:   format!(
                        "fn `{}` calls `{}`, which only `{trait_name}` can satisfy",
                        fact.function, place.candidate
                    ),
                    reason: "a written call is the author's stated intent".to_owned()
                }
            });
        }
    }
    found
}

/// Runs the full deduction for every derived intent, each installing
/// into the file that declares its type, the evidence standing as the
/// chain's first step.
#[must_use]
pub fn fulfill_intents(sources: &[Source]) -> Vec<Deduction> {
    let types = TypeGraph::from_sources(sources);
    derived_intents(sources)
        .into_iter()
        .filter_map(|intent| {
            let node = types.type_named(&intent.type_name)?;
            let mut held =
                deduce_impl(sources, node.source, &intent.type_name, &intent.trait_name);
            held.chain.steps.insert(0, intent.evidence);
            if let Some(choice) = held.catalog.as_mut() {
                choice.step += 1;
            }
            Some(held)
        })
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

    use super::{derived_intents, fulfill_intents};
    use crate::source::Source;

    fn one(text: &str) -> Vec<Source> {
        vec![Source::new(PathBuf::from("src/lib.rs"), text.to_owned())]
    }

    /// A written `default` call on a type without `Default` derives the
    /// intent, and its fulfilment reads back panic-free.
    #[test]
    fn a_written_default_call_derives_and_fulfils_the_intent() {
        let sources = one(
            "pub struct User {\n    pub name: String,\n    pub age: u8,\n}\n\npub fn make() -> \
             User {\n    User::default()\n}\n"
        );
        let intents = derived_intents(&sources);
        assert_eq!(intents.len(), 1, "{intents:?}");
        assert_eq!(intents[0].trait_name, "Default");
        assert!(intents[0].evidence.fact.contains("fn `make`"));
        let fulfilled = fulfill_intents(&sources);
        assert_eq!(fulfilled.len(), 1);
        let held = &fulfilled[0];
        assert!(held.gain.as_ref().is_some_and(|gain| !gain.panics));
        assert!(held.chain.steps[0].fact.contains("User::default"));
    }

    /// The catalog marker survives the evidence insertion: after the
    /// written call lands as step zero, the marker still names the very
    /// step that states the choice.
    #[test]
    fn the_catalog_marker_survives_the_evidence_insertion() {
        let sources = one(
            "pub struct User {\n    pub name: String,\n    pub age: u8,\n}\n\npub fn make() -> \
             User {\n    User::default()\n}\n"
        );
        let fulfilled = fulfill_intents(&sources);
        let held = &fulfilled[0];
        let marker = held.catalog.as_ref().unwrap();
        assert!(
            held.chain.steps[marker.step]
                .fact
                .contains("the catalog chooses"),
            "{:?}",
            held.chain.steps[marker.step]
        );
    }

    /// The evidence carries its written address: the byte range slices
    /// the citing source to the calling construct itself.
    #[test]
    fn the_evidence_carries_the_calls_written_address() {
        let text = "pub struct User {\n    pub name: String,\n    pub age: u8,\n}\n\npub fn make() \
                    -> User {\n    User::default()\n}\n";
        let sources = one(text);
        let intents = derived_intents(&sources);
        let held = &intents[0];
        assert_eq!(held.source, 0);
        let spelled = &text[held.site.start as usize..held.site.end as usize];
        assert!(spelled.contains("User::default"), "{spelled}");
    }

    /// A written `to_string` call asks for `Display` through the
    /// standard blanket.
    #[test]
    fn a_written_to_string_call_asks_for_display() {
        let sources = one(
            "pub struct Tag {\n    pub name: String,\n}\n\npub fn label(tag: &Tag) -> String \
             {\n    tag.to_string()\n}\n"
        );
        let intents = derived_intents(&sources);
        assert_eq!(intents.len(), 1, "{intents:?}");
        assert_eq!(intents[0].trait_name, "Display");
    }

    /// A derive whose contract writes the ability leaves no intent: a
    /// written `to_string` on an error-derived type is already served
    /// by the expansion's own `Display`.
    #[test]
    fn an_error_derive_leaves_no_display_intent() {
        let sources = one(
            "#[derive(Error)]\n#[error(\"broken\")]\npub struct Broken {\n    pub name: \
             String,\n}\n\npub fn label(held: &Broken) -> String {\n    held.to_string()\n}\n"
        );
        assert!(derived_intents(&sources).is_empty());
    }

    /// An ability already given leaves no intent to derive.
    #[test]
    fn a_given_ability_derives_no_intent() {
        let sources = one(
            "#[derive(Default)]\npub struct User {\n    pub name: String,\n}\n\npub fn make() -> \
             User {\n    User::default()\n}\n"
        );
        assert!(derived_intents(&sources).is_empty());
    }
}
