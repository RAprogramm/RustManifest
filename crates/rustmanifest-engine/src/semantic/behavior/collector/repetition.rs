// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The three repeating constructs, each visited with the multiplicity its
//! own body proves rather than the one its keyword suggests.

use syn::visit::Visit;

use super::{FactCollector, multiplicity::body_multiplicity};
use crate::semantic::behavior::output::Guard;

impl FactCollector {
    /// Binds the loop pattern against the iterated element type, then
    /// visits the iterable and the body.
    pub(super) fn collect_for_loop(&mut self, loop_expr: &syn::ExprForLoop) {
        self.bind_element(&loop_expr.pat, &loop_expr.expr);
        self.visit_expr(&loop_expr.expr);
        let body = self.flow_scope().block_flow(&loop_expr.body);
        let multiplicity = body_multiplicity(&body, loop_expr.label.as_ref());
        self.repeating(multiplicity, |collector| {
            collector.under(Guard::For, |collector| {
                collector.visit_block(&loop_expr.body);
            });
        });
    }

    /// A `while` re-evaluates its condition before every iteration, so a
    /// write standing in the condition repeats exactly as one in the
    /// body does. The condition still carries no guard: it is reached
    /// whenever the loop is.
    pub(super) fn collect_while_loop(&mut self, loop_expr: &syn::ExprWhile) {
        let body = self.flow_scope().block_flow(&loop_expr.body);
        let multiplicity = body_multiplicity(&body, loop_expr.label.as_ref());
        self.repeating(multiplicity, |collector| {
            collector.visit_expr(&loop_expr.cond);
            collector.under(Guard::While, |collector| {
                collector.visit_block(&loop_expr.body);
            });
        });
    }

    /// A bare `loop` repeats until something leaves it, and its body is
    /// entered unconditionally: the repetition is recorded while the
    /// guard stack stays as it was, so a write inside keeps the
    /// certainty of the position the loop stands in. How many times it
    /// repeats is the body's answer, not the loop keyword's.
    pub(super) fn collect_bare_loop(&mut self, loop_expr: &syn::ExprLoop) {
        let body = self.flow_scope().block_flow(&loop_expr.body);
        let multiplicity = body_multiplicity(&body, loop_expr.label.as_ref());
        self.repeating(multiplicity, |collector| {
            collector.visit_block(&loop_expr.body);
        });
    }
}
