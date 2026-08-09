// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Turning the indices one body records into a term that stands alone.
//!
//! Every index names a record of the same function, so unfolding is a
//! lookup and never a search. The budget is decremented at every
//! constructor, so the walk ends whatever the source does — a value
//! defined in terms of itself runs out of budget rather than out of
//! stack.

use super::composed::ComposedOrigin;
use crate::semantic::{
    behavior::{Arguments, FunctionFacts, Literal, Operation, OriginGap, Provenance},
    settled::Settled
};

/// Unfolds one origin of `fact` into a term that names no index.
///
/// The call term is left with its own answer unsettled: what a callee
/// hands back is the composition tier's question, and answering it here
/// would mean reading another function's facts from inside one function's
/// own unfolding.
pub(super) fn unfold(
    settled: Settled<'_>,
    fact: &FunctionFacts,
    origin: &Provenance,
    budget: u8
) -> ComposedOrigin {
    let Some(remaining) = budget.checked_sub(1) else {
        return ComposedOrigin::NotDerived(OriginGap::Deeper);
    };
    match origin {
        Provenance::Written(literal) => ComposedOrigin::Written(literal.clone()),
        Provenance::Parameter(slot) => ComposedOrigin::Parameter(slot.clone()),
        Provenance::Field(key) => ComposedOrigin::Field(key.clone()),
        Provenance::Constant(named) => ComposedOrigin::Constant(named.clone()),
        Provenance::Constructed(type_name) => ComposedOrigin::Constructed(type_name.clone()),
        Provenance::Supplied(named) => ComposedOrigin::Supplied(named.clone()),
        Provenance::Defaulted => ComposedOrigin::Defaulted,
        Provenance::Called(index) => called(settled, fact, *index, remaining),
        Provenance::Read(index) => drawn(fact, *index),
        Provenance::Computed(index) => computed(settled, fact, *index, remaining),
        Provenance::Chosen(index) => chosen(settled, fact, *index, remaining),
        Provenance::Leaves => ComposedOrigin::Leaves,
        Provenance::NotDerived(gap) => ComposedOrigin::NotDerived(*gap)
    }
}

/// Unfolds the read at one position of the input channel.
fn drawn(fact: &FunctionFacts, index: u32) -> ComposedOrigin {
    usize::try_from(index)
        .ok()
        .and_then(|at| fact.input_reads.get(at))
        .map_or(ComposedOrigin::NotDerived(OriginGap::Unreadable), |read| {
            ComposedOrigin::Boundary {
                channel:  read.channel,
                selector: read.selector.clone()
            }
        })
}

/// Unfolds the operation at one position of the operation channel.
fn computed(settled: Settled<'_>, fact: &FunctionFacts, index: u32, budget: u8) -> ComposedOrigin {
    let Some(site) = usize::try_from(index)
        .ok()
        .and_then(|at| fact.operations.get(at))
    else {
        return ComposedOrigin::NotDerived(OriginGap::Unreadable);
    };
    let operands: Vec<ComposedOrigin> = site
        .operands
        .iter()
        .map(|operand| unfold(settled, fact, operand, budget))
        .collect();
    ComposedOrigin::Computed {
        operation: site.operation.clone(),
        operands
    }
}

/// The half of a fallible answer a selection takes, where that is what
/// the selection does.
///
/// Read after the call it stands on has been settled and never before:
/// until then both halves are the same refusal, and collapsing to one
/// would keep that refusal after the answer arrived.
///
/// Taking a value apart with `Ok(value)` selects position zero of the
/// answer, and with `Err(error)` position one: the pattern names the
/// half, and the call already carries both. Restating it as a selection
/// over a call would put a step between a value and itself and lose
/// which half was taken.
pub(super) fn taken_half(
    operation: &Operation,
    operands: &[ComposedOrigin]
) -> Option<ComposedOrigin> {
    if !matches!(operation, Operation::Selection) {
        return None;
    }
    let [
        ComposedOrigin::Call {
            candidate,
            arguments,
            carried,
            failed,
            ..
        },
        ComposedOrigin::Written(Literal::Integer(position))
    ] = operands
    else {
        return None;
    };
    match position.as_str() {
        "0" => Some(carried.as_ref().clone()),
        "1" => {
            Some(written_failure(candidate, arguments).unwrap_or_else(|| failed.as_ref().clone()))
        }
        _ => None
    }
}

/// The failure half a library call is handed outright, where the library
/// takes it as an argument.
///
/// `value.ok_or(error)` hands back that very `error` when there is no
/// value: the failure half is written at the site and needs no callee to
/// answer for it. Only the spelling that takes the value itself is read
/// this way — one that takes a closure hands back what running the
/// closure produces, which is a different fact.
fn written_failure(candidate: &str, arguments: &[ComposedOrigin]) -> Option<ComposedOrigin> {
    candidate
        .rsplit("::")
        .next()
        .is_some_and(|named| named == "ok_or")
        .then(|| arguments.get(1).cloned())
        .flatten()
}

/// Unfolds the choice at one position of the choice channel.
fn chosen(settled: Settled<'_>, fact: &FunctionFacts, index: u32, budget: u8) -> ComposedOrigin {
    let Some(site) = usize::try_from(index)
        .ok()
        .and_then(|at| fact.choices.get(at))
    else {
        return ComposedOrigin::NotDerived(OriginGap::Unreadable);
    };
    ComposedOrigin::Choice(
        site.alternatives
            .iter()
            .map(|alternative| unfold(settled, fact, &alternative.value, budget))
            .collect()
    )
}

/// Unfolds the call at one position of the call channel.
fn called(settled: Settled<'_>, fact: &FunctionFacts, index: u32, budget: u8) -> ComposedOrigin {
    let Some(site) = usize::try_from(index)
        .ok()
        .and_then(|at| fact.call_sites.get(at))
    else {
        return ComposedOrigin::NotDerived(OriginGap::Unreadable);
    };
    let arguments = match &site.arguments {
        Arguments::Written(written) => written
            .iter()
            .map(|argument| unfold(settled, fact, argument, budget))
            .collect(),
        Arguments::Unwritten => Vec::new()
    };
    ComposedOrigin::Call {
        candidate: site.candidate.clone(),
        arguments,
        failed: Box::new(ComposedOrigin::NotDerived(match site.arguments {
            Arguments::Written(_) => OriginGap::CalleeUnresolved,
            Arguments::Unwritten => OriginGap::Unwritten
        })),
        carried: Box::new(ComposedOrigin::NotDerived(match site.arguments {
            Arguments::Written(_) => OriginGap::CalleeUnresolved,
            Arguments::Unwritten => OriginGap::Unwritten
        })),
        returns: settled.returns(fact, site.site)
    }
}
