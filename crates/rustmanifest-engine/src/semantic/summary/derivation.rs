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
use crate::semantic::behavior::{FunctionFacts, OriginGap, Outcome};

/// Everything one pass needs to settle a call.
pub(super) struct Bindings<'facts> {
    /// What every function is and how the calls between them resolve.
    pub(super) material:   super::Material<'facts>,
    /// Which functions share a recursion cycle.
    pub(super) components: &'facts Components,
    /// What a resolver settled about the sources, where one ran.
    pub(super) settled:    crate::semantic::settled::Settled<'facts>
}

/// Derives the summary of one function, settling every call it makes
/// against the summaries already derived.
pub(super) fn summary_of(
    bindings: &Bindings<'_>,
    fact: usize,
    settled: &BTreeMap<usize, FunctionSummary>
) -> FunctionSummary {
    let facts = bindings.material.facts;
    let Some(own) = facts.get(fact) else {
        return refused(fact, String::new(), Vec::new(), OriginGap::Unreadable);
    };
    let result = handed_back(bindings.settled, own, Half::Success);
    let failure = handed_back(bindings.settled, own, Half::Failure);
    FunctionSummary {
        function: own.function.clone(),
        fact,
        slots: own.slots.clone(),
        result: carry(bindings, fact, &result, settled, ORIGIN_DEPTH),
        failure: carry(bindings, fact, &failure, settled, ORIGIN_DEPTH),
        summary_set: SUMMARY_SET
    }
}

/// Which half of a fallible answer a term is being asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Half {
    /// The value a caller reads out of a successful answer.
    Success,
    /// The value handed back instead of it.
    Failure
}

/// The term one function's return positions amount to.
///
/// No position at all is a refusal, not an answer: the reading found no
/// return position, which is a statement about the reading. Several
/// positions are alternatives of one another, and are kept as such.
///
/// Where a function hands back both halves of a fallible answer, the
/// value a caller reads is the success half: the failure half is what it
/// hands back *instead*, and stating it as one of the values a caller
/// could be holding would let an error stand where a result was read.
/// Every position is still recorded, half by half, on the handback
/// channel — it is the summary alone that keeps to the half a reader
/// takes.
fn handed_back(
    settled: crate::semantic::settled::Settled<'_>,
    own: &FunctionFacts,
    half: Half
) -> ComposedOrigin {
    let carries_failure = own
        .handbacks
        .iter()
        .any(|handback| handback.outcome == Outcome::Failure);
    let mut terms: Vec<ComposedOrigin> = own
        .handbacks
        .iter()
        .filter(|handback| match half {
            Half::Success => !carries_failure || handback.outcome != Outcome::Failure,
            Half::Failure => handback.outcome == Outcome::Failure
        })
        .map(|handback| unfold(settled, own, &handback.value, ORIGIN_DEPTH))
        .collect();
    match terms.len() {
        0 => ComposedOrigin::NotDerived(OriginGap::Unreadable),
        1 => terms.remove(0),
        _ => ComposedOrigin::Choice(terms)
    }
}

/// Settles what every call inside a term hands back.
pub(super) fn carry(
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
            returns,
            ..
        } => {
            let written: Vec<ComposedOrigin> = arguments
                .iter()
                .map(|argument| carry(bindings, caller, argument, settled, remaining))
                .collect();
            let answered =
                callee_answer(bindings, caller, candidate, &written, settled, remaining);
            ComposedOrigin::Call {
                candidate: candidate.clone(),
                arguments: written,
                carried:   Box::new(answered.taken),
                failed:    Box::new(answered.instead),
                returns:   returns.clone()
            }
        }
        ComposedOrigin::Computed {
            operation,
            operands
        } => ComposedOrigin::Computed {
            operation: operation.clone(),
            operands:  operands
                .iter()
                .map(|operand| carry(bindings, caller, operand, settled, remaining))
                .collect()
        },
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
) -> Answered {
    let by_name = sole_by_name(bindings.material.facts);
    let Some(reached) = bound_callee(&by_name, bindings.material.qualifiers, candidate)
        .or_else(|| declared_in_file(bindings.material.facts, caller, candidate))
    else {
        return Answered::both(outside_or_unresolved(bindings, candidate));
    };
    if bindings.material.graph.possibilistic(caller, reached) {
        return Answered::both(ComposedOrigin::NotDerived(OriginGap::Dispatched));
    }
    if bindings.components.together(caller, reached) {
        return Answered::both(ComposedOrigin::NotDerived(OriginGap::Recursion));
    }
    let Some(summary) = settled.get(&reached) else {
        return Answered::both(ComposedOrigin::NotDerived(OriginGap::Recursion));
    };
    Answered {
        taken:   substitute(&summary.result, arguments, budget),
        instead: substitute(&summary.failure, arguments, budget)
    }
}

/// The two halves a call can hand back, in the caller's terms.
struct Answered {
    /// The value a caller reads out of a successful answer.
    taken:   ComposedOrigin,
    /// The value handed back instead of it.
    instead: ComposedOrigin
}

impl Answered {
    /// Both halves answered the same way, for the refusals that say
    /// nothing about which half was meant.
    fn both(term: ComposedOrigin) -> Self {
        Self {
            taken:   term.clone(),
            instead: term
        }
    }
}

/// The one function of the caller's own file that a bare candidate can
/// name.
///
/// An unqualified call reaches a name in scope, and an item written in
/// the same file is in scope. Where that file declares exactly one
/// function of the name, no other can stand at the position — a second
/// one would have to be imported, and the candidate would then be
/// spelled with its qualifier. Names that several files share are
/// settled by the file, not by the workspace.
fn declared_in_file(facts: &[FunctionFacts], caller: usize, candidate: &str) -> Option<usize> {
    if candidate.contains("::") {
        return None;
    }
    let source = facts.get(caller)?.source;
    let mut found = None;
    for (index, fact) in facts.iter().enumerate() {
        if fact.source != source || fact.function != candidate {
            continue;
        }
        if found.is_some() {
            return None;
        }
        found = Some(index);
    }
    found
}

/// Whether a candidate names something the workspace declares at all.
///
/// A name no function of the workspace carries is a call out of the
/// program: what it hands back is decided outside the sources being
/// read, which is a settled answer. A name the workspace does carry but
/// the binding rule could not tie to one definition is the engine's own
/// gap, and stays a refusal.
fn outside_or_unresolved(bindings: &Bindings<'_>, candidate: &str) -> ComposedOrigin {
    let named = candidate.rsplit("::").next().unwrap_or(candidate);
    let declared = bindings.material.facts.iter().any(|fact| {
        fact.function == named
            || fact
                .function
                .rsplit("::")
                .next()
                .is_some_and(|own| own == named)
    });
    if declared {
        return ComposedOrigin::NotDerived(OriginGap::CalleeUnresolved);
    }
    ComposedOrigin::Beyond(candidate.to_owned())
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
        failure: ComposedOrigin::NotDerived(gap),
        summary_set: SUMMARY_SET
    }
}
