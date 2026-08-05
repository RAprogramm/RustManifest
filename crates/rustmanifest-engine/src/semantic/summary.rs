// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a call to each function of a workspace hands back.
//!
//! One body proves where its own values came from, and stops at its own
//! edge: a value a call handed back is an index into that body's call
//! channel and nothing more. This tier carries the answer across that
//! edge.
//!
//! It rests on one property. A summary's only free variables are the
//! function's own inputs, so a caller substitutes the origin of the
//! argument it wrote at each position and is left with a term naming
//! nothing of the callee's. Substitutability is what makes provenance
//! compose; everything else here is the order in which to apply it.
//!
//! That order is the condensation of the call graph, which emits a group
//! only after every group it reaches. One pass therefore settles every
//! summary, with no fixpoint and no convergence argument. The one case a
//! single pass cannot settle — a callee in the caller's own cycle — is
//! refused by name, per position, so a caller that also calls a settled
//! helper keeps that helper's answer.

mod binding;
mod component;
mod composed;
mod derivation;
mod record;
mod substitution;
mod unfolding;

use std::collections::BTreeMap;

pub(in crate::semantic) use self::binding::{bound_callee, sole_by_name};
use self::{
    component::components,
    derivation::{Bindings, carry},
    unfolding::unfold
};
pub use self::{
    composed::{ComposedOrigin, ORIGIN_DEPTH},
    record::{FunctionSummary, SUMMARY_SET}
};
use crate::{
    semantic::{
        behavior::{FunctionFacts, Provenance},
        calls::CallGraph,
        meaning::qualifiers_by_fact
    },
    source::Source
};

/// What every function is and how the calls between them resolve.
///
/// The trio travels as one value because it is one: the facts, the graph
/// built over them, and the scopes those names are read in. Three
/// structures declared it separately before.
#[derive(Clone, Copy)]
pub(super) struct Material<'facts> {
    /// Every function's facts.
    pub(super) facts:      &'facts [FunctionFacts],
    /// The call graph the candidates were resolved against.
    pub(super) graph:      &'facts CallGraph,
    /// Scopes owning each function.
    pub(super) qualifiers: &'facts [std::collections::BTreeSet<String>]
}

/// Every function's summary, with the machinery to restate one value of
/// one function in terms that name nothing of any callee's.
pub struct Composition<'facts> {
    /// What every function is and how the calls between them resolve.
    material:   Material<'facts>,
    /// What each function hands back, by fact index.
    settled:    BTreeMap<usize, FunctionSummary>,
    /// Which functions share a recursion cycle.
    components: component::Components,
    /// The analyzed sources with what a resolver settled about them.
    settled_at: crate::semantic::settled::Settled<'facts>
}

impl Composition<'_> {
    /// Where one value of one function came from, with every reference
    /// followed and every call it passes through restated.
    #[must_use]
    pub fn of(&self, fact: usize, origin: &Provenance) -> ComposedOrigin {
        let Some(own) = self.material.facts.get(fact) else {
            return ComposedOrigin::NotDerived(crate::semantic::behavior::OriginGap::Unreadable);
        };
        let resolved = self.settled_at;
        let bindings = Bindings {
            material:   self.material,
            components: &self.components,
            settled:    resolved
        };
        let term = unfold(resolved, own, origin, composed::ORIGIN_DEPTH);
        carry(
            &bindings,
            fact,
            &term,
            &self.settled,
            composed::ORIGIN_DEPTH
        )
    }
}

/// Derives every function's summary and keeps what is needed to restate
/// one value in a caller's terms.
#[must_use]
pub fn composition<'facts>(
    facts: &'facts [FunctionFacts],
    graph: &'facts CallGraph,
    qualifiers: &'facts [std::collections::BTreeSet<String>]
) -> Composition<'facts> {
    composition_with(&[], None, facts, graph, qualifiers)
}

/// Composes as [`composition`] does, reading what a resolver settled about
/// `sources` where one ran.
#[must_use]
pub fn composition_with<'facts>(
    sources: &'facts [Source],
    resolution: Option<&'facts rustmanifest_schema::Resolution>,
    facts: &'facts [FunctionFacts],
    graph: &'facts CallGraph,
    qualifiers: &'facts [std::collections::BTreeSet<String>]
) -> Composition<'facts> {
    let components = components(graph, facts.len());
    let material = Material {
        facts,
        graph,
        qualifiers
    };
    let settled_at = crate::semantic::settled::Settled::new(resolution, sources);
    let bindings = Bindings {
        material,
        components: &components,
        settled: settled_at
    };
    let mut settled: BTreeMap<usize, FunctionSummary> = BTreeMap::new();
    for &fact in components.order() {
        let summary = derivation::summary_of(&bindings, fact, &settled);
        settled.insert(fact, summary);
    }
    Composition {
        material,
        settled,
        components,
        settled_at
    }
}

/// Derives what a call to every function of `facts` hands back.
///
/// Returned in fact order, so a summary is looked up by the same index
/// its facts carry.
#[must_use]
pub fn summaries(
    sources: &[Source],
    facts: &[FunctionFacts],
    graph: &CallGraph
) -> Vec<FunctionSummary> {
    summaries_with(sources, None, facts, graph)
}

/// Derives summaries as [`summaries`] does, reading what a resolver
/// settled about `sources` where one ran.
#[must_use]
pub fn summaries_with(
    sources: &[Source],
    resolution: Option<&rustmanifest_schema::Resolution>,
    facts: &[FunctionFacts],
    graph: &CallGraph
) -> Vec<FunctionSummary> {
    let qualifiers = qualifiers_by_fact(sources, facts);
    let components = components(graph, facts.len());
    let bindings = Bindings {
        material:   Material {
            facts,
            graph,
            qualifiers: &qualifiers
        },
        components: &components,
        settled:    crate::semantic::settled::Settled::new(resolution, sources)
    };
    let mut settled: BTreeMap<usize, FunctionSummary> = BTreeMap::new();
    for &fact in components.order() {
        let summary = derivation::summary_of(&bindings, fact, &settled);
        settled.insert(fact, summary);
    }
    (0..facts.len())
        .map(|fact| {
            settled.remove(&fact).unwrap_or_else(|| FunctionSummary {
                function: facts
                    .get(fact)
                    .map(|own| own.function.clone())
                    .unwrap_or_default(),
                fact,
                slots: Vec::new(),
                result: ComposedOrigin::NotDerived(
                    crate::semantic::behavior::OriginGap::Unreadable
                ),
                summary_set: SUMMARY_SET
            })
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

    use super::{ComposedOrigin, summaries};
    use crate::{
        semantic::{
            behavior::{Literal, OriginGap, behavior},
            calls::{call_graph, resolution_context}
        },
        source::Source
    };

    /// What a call to `function` hands back, in the caller's own terms.
    fn result(text: &str, function: &str) -> ComposedOrigin {
        let sources = [Source::new(PathBuf::from("src/lib.rs"), text.to_owned())];
        let facts = behavior(&sources);
        let context = resolution_context(&sources);
        let graph = call_graph(&facts, &context);
        summaries(&sources, &facts, &graph)
            .into_iter()
            .find(|summary| summary.function == function)
            .map(|summary| summary.result)
            .expect("the function has a summary")
    }

    /// The whole point of the tier: what a callee hands back is restated
    /// in terms of what the caller handed in.
    #[test]
    fn a_callee_answer_is_restated_in_the_caller_terms() {
        let carried = result(
            "fn inner(value: u32) -> u32 { value }\nfn outer() -> u32 { inner(7) }",
            "outer"
        );
        let ComposedOrigin::Call {
            candidate,
            carried,
            ..
        } = carried
        else {
            panic!("the outer function hands back the value a call produced: {carried:?}");
        };
        assert_eq!(candidate, "inner");
        assert_eq!(
            *carried,
            ComposedOrigin::Written(Literal::Integer("7".to_owned()))
        );
    }

    /// Three levels compose, which is the property a one-hop rule cannot
    /// deliver and the ordinary shape of a real program.
    #[test]
    fn a_three_level_chain_composes_all_the_way_down() {
        let carried = result(
            "fn deepest(value: u32) -> u32 { value }\nfn middle(value: u32) -> u32 { \
             deepest(value) }\nfn top() -> u32 { middle(5) }",
            "top"
        );
        let ComposedOrigin::Call {
            carried, ..
        } = carried
        else {
            panic!("the top function hands back a call's value: {carried:?}");
        };
        let ComposedOrigin::Call {
            carried: deeper, ..
        } = *carried
        else {
            panic!("the middle function hands back a call's value: {carried:?}");
        };
        assert_eq!(
            *deeper,
            ComposedOrigin::Written(Literal::Integer("5".to_owned()))
        );
    }

    /// A cycle is refused by name at the position that closes it, and the
    /// walk ends rather than iterating toward a fixpoint.
    #[test]
    fn a_recursion_cycle_is_refused_rather_than_iterated() {
        let carried = result(
            "fn ping(value: u32) -> u32 { pong(value) }\nfn pong(value: u32) -> u32 { ping(value) \
             }",
            "ping"
        );
        assert_eq!(carried.gap(), Some(OriginGap::Recursion));
    }

    /// A callee no single workspace function answers for is refused, not
    /// assumed transparent.
    #[test]
    fn an_unresolved_callee_is_refused() {
        let carried = result("fn caller() -> u32 { elsewhere::compute(1) }", "caller");
        assert_eq!(carried.gap(), Some(OriginGap::CalleeUnresolved));
    }

    /// A function writing no return position says the reading found none,
    /// which is not the same statement as handing nothing back.
    #[test]
    fn a_function_with_no_return_position_states_that_it_found_none() {
        assert_eq!(
            result("fn shout() { println!(\"hi\"); }", "shout").gap(),
            Some(OriginGap::Unreadable)
        );
    }
}
