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
use crate::semantic::behavior::{Arguments, FunctionFacts, OriginGap, Provenance};

/// Unfolds one origin of `fact` into a term that names no index.
///
/// The call term is left with its own answer unsettled: what a callee
/// hands back is the composition tier's question, and answering it here
/// would mean reading another function's facts from inside one function's
/// own unfolding.
pub(super) fn unfold(fact: &FunctionFacts, origin: &Provenance, budget: u8) -> ComposedOrigin {
    let Some(remaining) = budget.checked_sub(1) else {
        return ComposedOrigin::NotDerived(OriginGap::Deeper);
    };
    match origin {
        Provenance::Written(literal) => ComposedOrigin::Written(literal.clone()),
        Provenance::Parameter(slot) => ComposedOrigin::Parameter(slot.clone()),
        Provenance::Field(key) => ComposedOrigin::Field(key.clone()),
        Provenance::Called(index) => called(fact, *index, remaining),
        Provenance::Read(_) | Provenance::Computed(_) | Provenance::Chosen(_) => {
            ComposedOrigin::NotDerived(OriginGap::Unreadable)
        }
        Provenance::NotDerived(gap) => ComposedOrigin::NotDerived(*gap)
    }
}

/// Unfolds the call at one position of the call channel.
fn called(fact: &FunctionFacts, index: u32, budget: u8) -> ComposedOrigin {
    let Some(site) = usize::try_from(index)
        .ok()
        .and_then(|at| fact.call_sites.get(at))
    else {
        return ComposedOrigin::NotDerived(OriginGap::Unreadable);
    };
    let arguments = match &site.arguments {
        Arguments::Written(written) => written
            .iter()
            .map(|argument| unfold(fact, argument, budget))
            .collect(),
        Arguments::Unwritten => Vec::new()
    };
    ComposedOrigin::Call {
        candidate: site.candidate.clone(),
        arguments,
        carried: Box::new(ComposedOrigin::NotDerived(match site.arguments {
            Arguments::Written(_) => OriginGap::CalleeUnresolved,
            Arguments::Unwritten => OriginGap::Unwritten
        }))
    }
}
