// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Recording the conversion a library method performs.
//!
//! A conversion is a call whose produced type the caller decides: the
//! caller writes the target and the library produces it. That is what a
//! knowledge row states, and a call answering with a type the row fixes
//! transforms or accesses instead — reading one as a conversion would put
//! a type in the claim that the source never asked for.

use super::super::FactCollector;
use crate::semantic::behavior::{
    operation::{Conversion, Guarding, Operation},
    value::Provenance
};

impl FactCollector {
    /// Records the conversion a library method performs, where a
    /// knowledge row states that the caller decides what it produces.
    ///
    /// The target is the type the compiler settled for the call. A
    /// position it left unsettled is stated as unidentified — the call's
    /// written type argument named the spelling somebody reached the type
    /// by, which an alias or a renaming import makes a different name for
    /// the same answer.
    pub(super) fn record_conversion(
        &mut self,
        call: &syn::ExprMethodCall,
        operands: Vec<Provenance>
    ) -> bool {
        use syn::spanned::Spanned;

        let span = call.span().byte_range();
        let at = (
            u32::try_from(span.start).unwrap_or(u32::MAX),
            u32::try_from(span.end).unwrap_or(u32::MAX)
        );
        if !self.caller_decided.contains(&at) {
            return false;
        }
        let target = self.settled_type_over(&span);
        let guarding = match target
            .as_deref()
            .and_then(crate::semantic::behavior::outermost_name)
            .as_deref()
        {
            Some("Result") => Guarding::Reported,
            Some("Option") => Guarding::Checked,
            _ => Guarding::Unchecked
        };
        let index = self.push_operation(
            &call.span().byte_range(),
            Operation::Conversion(Conversion {
                target: target.clone()
            }),
            operands,
            guarding
        );
        if let Some(record) = self.facts.operations.get_mut(index) {
            record.produced = target;
        }
        true
    }
}
