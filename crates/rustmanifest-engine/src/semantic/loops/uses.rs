// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Use-shape classification of one binding within the rest of a loop
//! body.
//!
//! Every observed use falls into exactly one bucket; the rule layer
//! decides which buckets disqualify a candidate. Unclassifiable bare uses
//! and macro mentions are counted, never guessed at — the conservative
//! buckets are what keeps the loop rules free of false positives.

use rustmanifest_schema::{Location, Range, ResolvedMode, Resolution};
use syn::{spanned::Spanned, visit::Visit};

use crate::semantic::expr::{compound_assignment, place_root, token_words};

/// Classified uses of one binding within the rest of a loop body.
#[derive(Default)]
pub(super) struct UseSummary {
    /// Reads of a field of the binding.
    pub(super) field_reads:    u32,
    /// Shared borrows of the binding or one of its fields.
    pub(super) shared_borrows: u32,
    /// Mutable borrows of the binding or one of its fields.
    pub(super) mut_borrows:    u32,
    /// Assignments to the binding and calls taking it exclusively.
    pub(super) mutations:      u32,
    /// Calls the compiler says only borrow the binding.
    pub(super) calls:          u32,
    /// Calls taking the binding by value, and calls the resolver settled
    /// nothing about.
    pub(super) moves:          u32,
    /// Bare uses that could move the value (call argument, return, push).
    pub(super) bare:           u32,
    /// Mentions inside macro invocations, which hide the use shape.
    pub(super) macro_mentions: u32
}

/// Visitor classifying every use of one binding.
pub(super) struct UseCollector<'settled> {
    /// The binding under observation.
    pub(super) name:    String,
    /// Classified counts.
    pub(super) summary: UseSummary,
    /// What a resolver settled about the sources, where one ran.
    pub(super) settled: Option<&'settled Resolution>,
    /// File the visited body is written in, as the resolution keys it.
    pub(super) file:    &'settled std::path::Path
}

impl UseCollector<'_> {
    /// How the call written here takes its receiver, where the resolver
    /// settled it.
    fn takes(&self, call: &syn::ExprMethodCall) -> Option<ResolvedMode> {
        let bytes = call.span().byte_range();
        let at = Location {
            file:  self.file.to_path_buf(),
            range: Range {
                start: u32::try_from(bytes.start).unwrap_or(u32::MAX),
                end:   u32::try_from(bytes.end).unwrap_or(u32::MAX)
            }
        };
        self.settled
            .and_then(|settled| settled.at(&at))
            .and_then(|settled| settled.takes)
    }

    /// Returns whether `expr` is a bare path naming the binding.
    fn is_binding(&self, expr: &syn::Expr) -> bool {
        matches!(expr, syn::Expr::Path(path) if path.path.is_ident(self.name.as_str()))
    }

    /// Returns whether the root of a place expression is the binding.
    fn roots_in_binding(&self, expr: &syn::Expr) -> bool {
        place_root(expr).is_some_and(|root| root == self.name)
    }
}

impl<'ast> Visit<'ast> for UseCollector<'_> {
    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        if self.is_binding(&call.receiver) {
            match self.takes(call) {
                Some(ResolvedMode::Borrowed) => self.summary.calls += 1,
                Some(ResolvedMode::BorrowedMut) => self.summary.mutations += 1,
                Some(ResolvedMode::Owned) | None => self.summary.moves += 1
            }
            for argument in &call.args {
                self.visit_expr(argument);
            }
            return;
        }
        syn::visit::visit_expr_method_call(self, call);
    }

    fn visit_expr_field(&mut self, field: &'ast syn::ExprField) {
        if self.is_binding(&field.base) {
            self.summary.field_reads += 1;
            return;
        }
        syn::visit::visit_expr_field(self, field);
    }

    fn visit_expr_reference(&mut self, reference: &'ast syn::ExprReference) {
        if self.roots_in_binding(&reference.expr) {
            if reference.mutability.is_some() {
                self.summary.mut_borrows += 1;
            } else {
                self.summary.shared_borrows += 1;
            }
            return;
        }
        syn::visit::visit_expr_reference(self, reference);
    }

    fn visit_expr_assign(&mut self, assignment: &'ast syn::ExprAssign) {
        if self.roots_in_binding(&assignment.left) {
            self.summary.mutations += 1;
            self.visit_expr(&assignment.right);
            return;
        }
        syn::visit::visit_expr_assign(self, assignment);
    }

    fn visit_expr_binary(&mut self, binary: &'ast syn::ExprBinary) {
        if compound_assignment(binary.op) && self.roots_in_binding(&binary.left) {
            self.summary.mutations += 1;
            self.visit_expr(&binary.right);
            return;
        }
        syn::visit::visit_expr_binary(self, binary);
    }

    fn visit_expr_path(&mut self, path: &'ast syn::ExprPath) {
        if path.path.is_ident(self.name.as_str()) {
            self.summary.bare += 1;
        }
        syn::visit::visit_expr_path(self, path);
    }

    fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
        if token_words(&invocation.tokens.to_string()).contains(&self.name) {
            self.summary.macro_mentions += 1;
        }
        syn::visit::visit_macro(self, invocation);
    }
}
