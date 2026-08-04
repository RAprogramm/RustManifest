// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Recording one position that hands a value back to the caller.

use super::FactCollector;
use crate::semantic::behavior::{
    handback::{Handback, Outcome},
    value::Provenance
};

/// The two variants a fallible return is written as, with the half each
/// one carries.
const OUTCOME_VARIANTS: [(&str, Outcome); 2] =
    [("Ok", Outcome::Success), ("Err", Outcome::Failure)];

impl FactCollector {
    /// Records one position handing a value back, in body order.
    fn push_handback(
        &mut self,
        span: &std::ops::Range<usize>,
        value: Provenance,
        outcome: Outcome
    ) {
        self.facts.handbacks.push(Handback {
            value,
            outcome,
            site: rustmanifest_schema::Range {
                start: u32::try_from(span.start).unwrap_or(u32::MAX),
                end:   u32::try_from(span.end).unwrap_or(u32::MAX)
            },
            guard: self.site_guard(),
            conditions: self.site_conditions()
        });
    }

    /// Records a written `return`, whose operand is the value handed back
    /// and whose absence is the unit value.
    pub(super) fn collect_return(&mut self, returned: &syn::ExprReturn) {
        use syn::spanned::Spanned;

        syn::visit::visit_expr_return(self, returned);
        let span = returned.span().byte_range();
        match &returned.expr {
            Some(written) => {
                let value = self.origin_of(written);
                let outcome = outcome_of(written);
                self.push_handback(&span, value, outcome);
            }
            None => self.push_handback(
                &span,
                Provenance::Written(crate::semantic::behavior::value::Literal::Unit),
                Outcome::Plain
            )
        }
    }

    /// Records the failure a `?` hands back.
    ///
    /// The value is the origin of the operand, which is the fallible value
    /// itself: what leaves the function is that value's failure half, and
    /// naming the operand is the closest the source licenses without
    /// taking a value apart that was never written apart.
    pub(super) fn record_try_handback(&mut self, tried: &syn::ExprTry) {
        use syn::spanned::Spanned;

        let span = tried.span().byte_range();
        let value = self.origin_of(&tried.expr);
        self.push_handback(&span, value, Outcome::Failure);
    }

    /// Records the tail expression of a function body, which hands its
    /// value back without writing a `return`.
    ///
    /// A body whose last statement is not a tail expression hands back
    /// whatever the language gives it, and that is not a position the
    /// source writes: nothing is recorded rather than a position invented
    /// at the closing brace.
    pub(in crate::semantic::behavior) fn collect_tail(&mut self, block: &syn::Block) {
        use syn::spanned::Spanned;

        let Some(syn::Stmt::Expr(written, None)) = block.stmts.last() else {
            return;
        };
        if matches!(written, syn::Expr::Return(_)) {
            return;
        }
        let span = written.span().byte_range();
        let value = self.origin_of(written);
        let outcome = outcome_of(written);
        self.push_handback(&span, value, outcome);
    }
}

/// Which half of a fallible return a written expression hands back, where
/// the source writes the variant outright.
fn outcome_of(expression: &syn::Expr) -> Outcome {
    let syn::Expr::Call(call) = expression else {
        return Outcome::Plain;
    };
    let syn::Expr::Path(path) = call.func.as_ref() else {
        return Outcome::Plain;
    };
    let Some(named) = path.path.segments.last() else {
        return Outcome::Plain;
    };
    let named = named.ident.to_string();
    OUTCOME_VARIANTS
        .iter()
        .find(|(variant, _)| *variant == named.as_str())
        .map_or(Outcome::Plain, |(_, outcome)| *outcome)
}
