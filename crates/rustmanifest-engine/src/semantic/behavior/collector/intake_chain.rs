// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Following a value from outside the process through the constructs
//! written over it.
//!
//! A boundary is ordinarily written with something over it: what a
//! process was started with is collected, an element of it is singled
//! out, and only then is the value used. Each of those is a method whose
//! receiver types to nothing, so without this reading the boundary is
//! lost at the first one and the site records nothing but a method on an
//! unknown receiver.
//!
//! Following one does not add a record. The read is one event and stays
//! one record; a construct written over it either leaves the part drawn
//! as it was, singles one out, or leaves which part is drawn unproven
//! while the channel stays proven. That is the same field-by-field
//! degradation the write channel practises on a payload.
//!
//! A refusal already recorded is never taken back. A position singled out
//! of a sequence something has already advanced is a position of that
//! sequence and not of the channel, so the part drawn stays unproven
//! while the channel stays proven.

use super::{
    FactCollector,
    catalog::intake::{BOUNDARY_ADAPTERS, ELEMENT_ADAPTERS}
};
use crate::semantic::behavior::{
    input::{ReadShape, Selector, SelectorGap},
    value::Provenance
};

impl FactCollector {
    /// Folds a construct written over a read into that read, answering
    /// with whether it was one.
    ///
    /// The record the construct stands over is found by the origin of its
    /// receiver, so a chain of any length is followed one link at a time
    /// and nothing is searched for.
    ///
    /// Two constructs singling out different positions of one source are
    /// two reads, not one narrowed twice. Folding the second into the
    /// first would report the position the first asked for and call it the
    /// second's — a false statement, which is worse than a refusal.
    ///
    /// An adapter that does not keep positions costs the read its proven
    /// selector — but only while the read still stands for a sequence.
    /// Once a written position singled one element out, there is no
    /// sequence left to re-order: what follows transforms that element and
    /// cannot change which one it was. Degrading the selector there would
    /// throw away a fact the source had already settled.
    pub(super) fn follow_intake(&mut self, call: &syn::ExprMethodCall) -> bool {
        let method = call.method.to_string();
        let Provenance::Read(index) = self.origin_of(&call.receiver) else {
            return false;
        };
        let Some(read) = usize::try_from(index)
            .ok()
            .and_then(|at| self.facts.input_reads.get(at))
            .cloned()
        else {
            return false;
        };
        let mut narrowed = read;
        let mut settled = index;
        if let Some((_, keeps)) = BOUNDARY_ADAPTERS
            .iter()
            .find(|(named, _)| *named == method.as_str())
        {
            if !keeps && narrowed.shape != ReadShape::Element {
                narrowed.selector = Selector::NotIdentified(SelectorGap::Advanced);
            }
        } else if ELEMENT_ADAPTERS.contains(&method.as_str()) {
            let asked = position_of(call.args.first());
            if narrowed.shape == ReadShape::Element && narrowed.selector != asked {
                narrowed.selector = asked;
                settled = u32::try_from(self.facts.input_reads.len()).unwrap_or(u32::MAX);
                self.facts.input_reads.push(narrowed);
                return self.settle_intake(call, settled);
            }
            if narrowed.selector == Selector::Whole {
                narrowed.selector = asked;
            }
            narrowed.shape = ReadShape::Element;
        } else {
            return false;
        }
        if let Some(record) = usize::try_from(index)
            .ok()
            .and_then(|at| self.facts.input_reads.get_mut(at))
        {
            *record = narrowed;
        }
        self.settle_intake(call, settled)
    }

    /// Names the read a folded construct stands for, at that construct's
    /// own position.
    fn settle_intake(&mut self, call: &syn::ExprMethodCall, read: u32) -> bool {
        use syn::spanned::Spanned;

        let span = call.span().byte_range();
        self.produced.insert(
            (
                u32::try_from(span.start).unwrap_or(u32::MAX),
                u32::try_from(span.end).unwrap_or(u32::MAX)
            ),
            Provenance::Read(read)
        );
        true
    }
}

/// Which element of a sequence a construct singles out, where the source
/// writes the position as a number.
fn position_of(argument: Option<&syn::Expr>) -> Selector {
    let Some(written) = argument else {
        return Selector::NotIdentified(SelectorGap::RuntimeValue);
    };
    let syn::Expr::Lit(literal) = written else {
        return Selector::NotIdentified(SelectorGap::RuntimeValue);
    };
    let syn::Lit::Int(position) = &literal.lit else {
        return Selector::NotIdentified(SelectorGap::RuntimeValue);
    };
    position
        .base10_parse::<u32>()
        .map_or(Selector::NotIdentified(SelectorGap::RuntimeValue), |at| {
            Selector::Positional(at)
        })
}
