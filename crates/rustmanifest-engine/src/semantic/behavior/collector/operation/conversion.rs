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
use crate::semantic::{
    behavior::{
        operation::{Conversion, Guarding, Operation},
        value::Provenance
    },
    knowledge::{Outcome, converts},
    std_knowledge::method_result
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

        let method = call.method.to_string();
        let owner = self.expr_type(&call.receiver);
        let Some(row) = owner
            .as_deref()
            .and_then(|owner| method_result(owner, &method))
        else {
            return false;
        };
        if !converts(&row) {
            return false;
        }
        let target = self.settled_type_over(&call.span().byte_range());
        let guarding = match row.outcome {
            Outcome::Total => Guarding::Unchecked,
            Outcome::Fallible => Guarding::Reported,
            Outcome::Optional => Guarding::Checked
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
