// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Restating a callee's answer in the caller's own terms.
//!
//! A summary's only free variables are the callee's own inputs, so
//! substituting the origin of the argument written at each position
//! leaves a term naming nothing of the callee's. That property is the
//! whole of why provenance crosses a call.
//!
//! A position the site wrote no argument for is refused rather than
//! dropped: a term that quietly kept the callee's own input would read as
//! the caller's value and name something the caller never handed over.

use super::composed::ComposedOrigin;
use crate::semantic::behavior::OriginGap;

/// Replaces every input of a callee's term with the origin the site wrote
/// at that position.
pub(super) fn substitute(
    term: &ComposedOrigin,
    arguments: &[ComposedOrigin],
    budget: u8
) -> ComposedOrigin {
    let Some(remaining) = budget.checked_sub(1) else {
        return ComposedOrigin::NotDerived(OriginGap::Deeper);
    };
    match term {
        ComposedOrigin::Parameter(slot) => arguments
            .get(usize::from(slot.position))
            .cloned()
            .unwrap_or(ComposedOrigin::NotDerived(OriginGap::Unwritten)),
        ComposedOrigin::Call {
            candidate,
            arguments: written,
            carried,
            returns
        } => ComposedOrigin::Call {
            candidate: candidate.clone(),
            arguments: written
                .iter()
                .map(|inner| substitute(inner, arguments, remaining))
                .collect(),
            carried:   Box::new(substitute(carried, arguments, remaining)),
            returns:   returns.clone()
        },
        ComposedOrigin::Computed {
            operation,
            operands
        } => ComposedOrigin::Computed {
            operation: operation.clone(),
            operands:  operands
                .iter()
                .map(|operand| substitute(operand, arguments, remaining))
                .collect()
        },
        ComposedOrigin::Choice(alternatives) => ComposedOrigin::Choice(
            alternatives
                .iter()
                .map(|alternative| substitute(alternative, arguments, remaining))
                .collect()
        ),
        settled => settled.clone()
    }
}
