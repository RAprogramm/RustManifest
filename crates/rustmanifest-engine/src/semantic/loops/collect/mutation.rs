// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Roots a loop body may change: assignment targets, `&mut` borrows,
//! mutating or consuming method receivers, and every identifier a macro
//! mentions.
//!
//! The set is deliberately over-approximated. A macro can move or mutate
//! anything it names, so every name it mentions counts as mutated; a
//! clone whose receiver appears here is never called loop-invariant.

use syn::visit::Visit;

use crate::semantic::expr::{
    MUTATING_METHODS, compound_assignment, consuming_method, place_root, token_words
};

/// Visitor collecting root identifiers that a loop body may mutate.
pub(super) struct MutatedCollector {
    /// Roots of assignment targets, `&mut` borrows, mutating or consuming
    /// method receivers, and every identifier a macro mentions.
    pub(super) roots: Vec<String>
}

impl MutatedCollector {
    /// Records the root of a place expression, when it has one.
    fn record(&mut self, expr: &syn::Expr) {
        if let Some(root) = place_root(expr) {
            self.roots.push(root);
        }
    }
}

impl<'ast> Visit<'ast> for MutatedCollector {
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
        let method = call.method.to_string();
        if MUTATING_METHODS.contains(&method.as_str()) || consuming_method(&method) {
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
