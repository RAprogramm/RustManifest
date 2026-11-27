// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Roots a loop body may change: assignment targets, `&mut` borrows,
//! receivers of calls the compiler says take them exclusively or by
//! value, and every identifier a macro mentions.
//!
//! The set is deliberately over-approximated. A macro can move or mutate
//! anything it names, so every name it mentions counts as mutated; a
//! clone whose receiver appears here is never called loop-invariant. A
//! call the resolver did not settle counts the same way: what a call does
//! to its receiver is the receiver's declared mode, and where that answer
//! is missing the receiver is treated as changed rather than guessed from
//! the method's spelling.

use rustmanifest_schema::{Resolution, ResolvedMode};
use syn::{spanned::Spanned, visit::Visit};

use crate::semantic::expr::{compound_assignment, place_root, settled_call, token_words};

/// Visitor collecting root identifiers that a loop body may mutate.
pub(super) struct MutatedCollector<'settled> {
    /// Roots of assignment targets, `&mut` borrows, exclusively or owned
    /// taken receivers, and every identifier a macro mentions.
    pub(super) roots:   Vec<String>,
    /// What a resolver settled about the sources, where one ran.
    pub(super) settled: Option<&'settled Resolution>,
    /// File the visited body is written in, as the resolution keys it.
    pub(super) file:    &'settled std::path::Path
}

impl MutatedCollector<'_> {
    /// Records the root of a place expression, when it has one.
    fn record(&mut self, expr: &syn::Expr) {
        if let Some(root) = place_root(expr) {
            self.roots.push(root);
        }
    }

    /// Whether the call may change what it is written on: the receiver is
    /// taken exclusively or by value, or the resolver settled nothing
    /// about the call at all.
    fn may_change_receiver(&self, call: &syn::ExprMethodCall) -> bool {
        settled_call(self.settled, self.file, call.span())
            .and_then(|settled| settled.takes)
            .is_none_or(|takes| matches!(takes, ResolvedMode::BorrowedMut | ResolvedMode::Owned))
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
        if self.may_change_receiver(call) {
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
