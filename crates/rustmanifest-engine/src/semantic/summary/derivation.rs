// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Deriving one function's summary, and settling what each call it makes
//! hands back.
//!
//! Settled in one pass over the condensation order, so a callee's summary
//! exists before its caller's is derived. Every question the pass cannot
//! settle is a leaf naming that question: a candidate no single function
//! answers, a candidate outside the workspace, a callee in the caller's
//! own cycle, a dispatch the graph has already declared uncertain.

use std::collections::BTreeMap;

use super::{
    binding::{bound_callee, sole_by_name},
    component::Components,
    composed::{ComposedOrigin, ORIGIN_DEPTH},
    record::{FunctionSummary, SUMMARY_SET},
    substitution::substitute,
    unfolding::unfold
};
use crate::semantic::{
    behavior::{FunctionFacts, OriginGap},
    calls::CallGraph
};

/// Everything one pass needs to settle a call.
pub(super) struct Bindings<'facts> {
    /// Every function's facts.
    pub(super) facts:      &'facts [FunctionFacts],
    /// The call graph the candidates were resolved against.
    pub(super) graph:      &'facts CallGraph,
    /// Scopes owning each function, for the qualified binding rule.
    pub(super) qualifiers: &'facts [std::collections::BTreeSet<String>],
    /// Which functions share a recursion cycle.
    pub(super) components: &'facts Components
}

/// Derives the summary of one function, settling every call it makes
/// against the summaries already derived.
pub(super) fn summary_of(
    bindings: &Bindings<'_>,
    fact: usize,
    settled: &BTreeMap<usize, FunctionSummary>
) -> FunctionSummary {
    let facts = bindings.facts;
    let Some(own) = facts.get(fact) else {
        return refused(fact, String::new(), Vec::new(), OriginGap::Unreadable);
    };
    let result = handed_back(own);
    let carried = carry(bindings, fact, &result, settled, ORIGIN_DEPTH);
    FunctionSummary {
        function: own.function.clone(),
        fact,
        slots: own.slots.clone(),
        result: carried,
        summary_set: SUMMARY_SET
    }
}

/// The term one function's return positions amount to.
///
/// No position at all is a refusal, not an answer: the reading found no
/// return position, which is a statement about the reading. Several
/// positions are alternatives of one another, and are kept as such.
fn handed_back(own: &FunctionFacts) -> ComposedOrigin {
    let mut terms: Vec<ComposedOrigin> = own
        .handbacks
        .iter()
        .map(|handback| unfold(own, &handback.value, ORIGIN_DEPTH))
        .collect();
    match terms.len() {
        0 => ComposedOrigin::NotDerived(OriginGap::Unreadable),
        1 => terms.remove(0),
        _ => ComposedOrigin::Choice(terms)
    }
}

/// Settles what every call inside a term hands back.
fn carry(
    bindings: &Bindings<'_>,
    caller: usize,
    term: &ComposedOrigin,
    settled: &BTreeMap<usize, FunctionSummary>,
    budget: u8
) -> ComposedOrigin {
    let Some(remaining) = budget.checked_sub(1) else {
        return ComposedOrigin::NotDerived(OriginGap::Deeper);
    };
    match term {
        ComposedOrigin::Call {
            candidate,
            arguments,
            ..
        } => {
            let written: Vec<ComposedOrigin> = arguments
                .iter()
                .map(|argument| carry(bindings, caller, argument, settled, remaining))
                .collect();
            let answer = callee_answer(bindings, caller, candidate, &written, settled, remaining);
            ComposedOrigin::Call {
                candidate: candidate.clone(),
                arguments: written,
                carried:   Box::new(answer)
            }
        }
        ComposedOrigin::Choice(alternatives) => ComposedOrigin::Choice(
            alternatives
                .iter()
                .map(|alternative| carry(bindings, caller, alternative, settled, remaining))
                .collect()
        ),
        settled_term => settled_term.clone()
    }
}

/// What the callee of one candidate hands back, in the caller's terms.
fn callee_answer(
    bindings: &Bindings<'_>,
    caller: usize,
    candidate: &str,
    arguments: &[ComposedOrigin],
    settled: &BTreeMap<usize, FunctionSummary>,
    budget: u8
) -> ComposedOrigin {
    let by_name = sole_by_name(bindings.facts);
    let Some(reached) = bound_callee(&by_name, bindings.qualifiers, candidate) else {
        return ComposedOrigin::NotDerived(OriginGap::CalleeUnresolved);
    };
    if bindings.graph.possibilistic(caller, reached) {
        return ComposedOrigin::NotDerived(OriginGap::Dispatched);
    }
    if bindings.components.together(caller, reached) {
        return ComposedOrigin::NotDerived(OriginGap::Recursion);
    }
    let Some(summary) = settled.get(&reached) else {
        return ComposedOrigin::NotDerived(OriginGap::Recursion);
    };
    substitute(&summary.result, arguments, budget)
}

/// A summary of a function whose facts could not be read.
const fn refused(
    fact: usize,
    function: String,
    slots: Vec<Option<String>>,
    gap: OriginGap
) -> FunctionSummary {
    FunctionSummary {
        function,
        fact,
        slots,
        result: ComposedOrigin::NotDerived(gap),
        summary_set: SUMMARY_SET
    }
}
