// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Roots a loop body may change: assignment targets, `&mut` borrows, the
//! receivers of calls declared to take them mutably or by value, and every
//! identifier a macro mentions.
//!
//! Which calls those are is the mode the function reached declares, as a
//! resolver states it — not a list of spellings that usually mutate.
//!
//! The set is deliberately over-approximated. A macro can move or mutate
//! anything it names, so every name it mentions counts as mutated; a
//! clone whose receiver appears here is never called loop-invariant.
//!
//! A call nothing settled a mode for is not counted here. The binding it
//! stands on is judged by its own uses, where an unsettled call is its own
//! bucket and refuses the rule outright — counting it twice would refuse
//! the clone that defines a candidate on the strength of the clone itself.

use syn::{spanned::Spanned as _, visit::Visit};

use crate::semantic::{
    behavior::ParamMode,
    expr::{compound_assignment, place_root, token_words}
};

/// Visitor collecting root identifiers that a loop body may mutate.
pub(super) struct MutatedCollector<'takes> {
    /// Roots of assignment targets, `&mut` borrows, mutating or consuming
    /// method receivers, and every identifier a macro mentions.
    pub(super) roots: Vec<String>,
    /// How every settled call of this source takes the value it is
    /// written on.
    pub(super) takes: &'takes super::Taken
}

impl MutatedCollector<'_> {
    /// Records the root of a place expression, when it has one.
    fn record(&mut self, expr: &syn::Expr) {
        if let Some(root) = place_root(expr) {
            self.roots.push(root);
        }
    }

    /// How the call written here takes the value it is written on, where
    /// a resolver settled it.
    fn mode(&self, call: &syn::ExprMethodCall) -> Option<ParamMode> {
        let span = call.span().byte_range();
        let start = u32::try_from(span.start).ok()?;
        let end = u32::try_from(span.end).ok()?;
        self.takes.get(&(start, end)).copied()
    }
}

impl<'ast> Visit<'ast> for MutatedCollector<'_> {
    fn visit_expr_assign(&mut self, assignment: &'ast syn::ExprAssign) {
        self.record(&assignment.left);
        syn::visit::visit_expr_assign(self, assignment);
    }

    fn visit_expr_binary(&mut self, binary: &'ast syn::ExprBinary) {
        if compound_assignment(binary.op) {
            self.record(&binary.left);
        }
        syn::visit::visit_expr_binary(self, binary);
    }

    fn visit_expr_reference(&mut self, reference: &'ast syn::ExprReference) {
        if reference.mutability.is_some() {
            self.record(&reference.expr);
        }
        syn::visit::visit_expr_reference(self, reference);
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        if matches!(
            self.mode(call),
            Some(ParamMode::BorrowedMut | ParamMode::Owned)
        ) {
            self.record(&call.receiver);
        }
        syn::visit::visit_expr_method_call(self, call);
    }

    fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
        self.roots
            .extend(token_words(&invocation.tokens.to_string()));
        syn::visit::visit_macro(self, invocation);
    }
}
