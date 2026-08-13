// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The identity a compiler settles over a fixture: what each declared
//! body is called, and which body each written call reaches.
//!
//! The call graph reads identity and nothing else — a body is known by the
//! path that reaches it, a call by the path it reaches — so a test whose
//! subject is what the graph is *used for* has to state that identity
//! before it can ask anything. Stating it row by row would bury the
//! subject; for a fixture of a few lines, where every written name has one
//! declaration, the identity is the written name, and this states it once
//! for all of them.
//!
//! A name several bodies carry is left unsettled here rather than picked
//! between: which of them a call means is exactly the question a resolver
//! answers and a fixture helper must not pretend to.
//!
//! Nothing is stated about the *type* a call hands back: the rows carry an
//! empty rendering, which every reader takes as no answer.

use std::collections::BTreeMap;

use rustmanifest_schema::{Location, Resolution, ResolvedCall, ResolvedFunction};

use crate::{semantic::behavior::FunctionFacts, source::Source};

/// Adds to `resolution` the identity a compiler settles for `facts`.
pub fn settle_identity(
    resolution: &mut Resolution,
    sources: &[Source],
    facts: &[FunctionFacts]
) {
    let declared = declaration_counts(facts);
    for fact in facts {
        let Some(file) = sources.get(fact.source).map(|source| source.path().to_path_buf()) else {
            continue;
        };
        resolution.functions.push(ResolvedFunction {
            at:    Location {
                file:  file.clone(),
                range: fact.range
            },
            names: fact.function.clone()
        });
        for site in &fact.call_sites {
            let Some(reached) = reached_body(&declared, &site.candidate) else {
                continue;
            };
            if site.site.end <= site.site.start {
                continue;
            }
            resolution.calls.push(ResolvedCall {
                at:       Location {
                    file:  file.clone(),
                    range: site.site
                },
                callee:   reached.rsplit("::").next().unwrap_or(reached).to_owned(),
                returns:  String::new(),
                receiver: None,
                reaches:  Some(reached.to_owned()),
                takes:    None
            });
        }
    }
    resolution.settle();
}

/// The identity a compiler settles for `facts`, and nothing else.
#[must_use]
pub fn identity_of(sources: &[Source], facts: &[FunctionFacts]) -> Resolution {
    let mut resolution = Resolution {
        resolver: "test".to_owned(),
        ..Resolution::default()
    };
    settle_identity(&mut resolution, sources, facts);
    resolution
}

/// The identity a compiler settles for what `sources` declare.
///
/// The facts are read once without a resolution to find where the bodies
/// and the call sites stand; nothing about a type is taken from that
/// reading, only the places.
#[must_use]
pub fn identity_over(sources: &[Source]) -> Resolution {
    identity_of(sources, &crate::semantic::behavior::behavior(sources))
}

/// The body one written candidate reaches, where the fixture leaves no
/// room for a second answer.
///
/// A candidate already naming a declared body reaches it. One qualified by
/// a module — `crate::domain::decide` is written down as `domain::decide`
/// — reaches the body of that bare name, provided no type of the
/// qualifier's spelling declares members at all. A receiver the reading
/// could not type reaches nothing: which body it means is the question,
/// not something a fixture may assume.
fn reached_body<'name>(
    declared: &BTreeMap<&'name str, usize>,
    candidate: &'name str
) -> Option<&'name str> {
    if declared.get(candidate) == Some(&1) {
        return Some(candidate);
    }
    let (owner, member) = candidate.rsplit_once("::")?;
    if owner == "?" || declared.keys().any(|named| named.starts_with(&format!("{owner}::"))) {
        return None;
    }
    declared
        .get_key_value(member)
        .and_then(|(named, count)| (*count == 1).then_some(*named))
}

/// How many bodies each written name is carried by.
fn declaration_counts(facts: &[FunctionFacts]) -> BTreeMap<&str, usize> {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for fact in facts {
        *counts.entry(fact.function.as_str()).or_default() += 1;
    }
    counts
}
