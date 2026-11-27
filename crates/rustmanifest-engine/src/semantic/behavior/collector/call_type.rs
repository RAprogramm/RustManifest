// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Call typing: the return-table key a call expression normalizes to, and
//! the type name that call hands back once the unwrapping written around
//! it is read.

use super::FactCollector;

impl FactCollector {
    /// How the call written here takes the value it stands on.
    pub(super) fn settled_receiver_mode(
        &self,
        call: &syn::ExprMethodCall
    ) -> Option<crate::semantic::behavior::types::ParamMode> {
        use syn::spanned::Spanned;

        let span = call.span().byte_range();
        let start = u32::try_from(span.start).ok()?;
        let end = u32::try_from(span.end).ok()?;
        self.takes.get(&(start, end)).copied()
    }

    /// The type a resolver settled for the receiver of the call at this
    /// position.
    ///
    /// The resolver answers what written text cannot — which function a
    /// call reaches and what it hands back — and its answer outranks
    /// every reading below, which infers from spelling alone. A position
    /// the resolver left out is absent from the overlay, so the reading
    /// falls through to what the source itself proves.
    pub(super) fn settled_receiver(&self, expression: &syn::Expr) -> Option<String> {
        use syn::spanned::Spanned;

        let span = expression.span().byte_range();
        let start = u32::try_from(span.start).ok()?;
        let end = u32::try_from(span.end).ok()?;
        self.receivers.get(&(start, end)).cloned()
    }

    /// The type a resolver settled for the expression at this position.
    fn settled_type(&self, expression: &syn::Expr) -> Option<String> {
        use syn::spanned::Spanned;

        let span = expression.span().byte_range();
        let start = u32::try_from(span.start).ok()?;
        let end = u32::try_from(span.end).ok()?;
        self.settled.get(&(start, end)).cloned()
    }

    /// Types an expression that calls a function with a known return: a
    /// The value a call hands back, as the compiler settled it.
    ///
    /// A call's answer is its own type, stated at the place it is
    /// written. Walking back to the callee's declaration, unwrapping a
    /// result by name and following a chain receiver-first were three
    /// ways of arriving at the same answer for the callees this workspace
    /// happened to declare.
    pub(super) fn called_return(&self, expression: &syn::Expr) -> Option<String> {
        self.settled_type(expression)
    }
}
