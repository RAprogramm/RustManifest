// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The syntax-directed reachability fold over expressions: every
//! construct that can end a path has a rule here, and everything else
//! folds over its children.

use super::{completion::Completion, flow::Flow, scope::FlowScope, unwrapping::unwrapped};

impl FlowScope<'_> {
    /// What an expression proves about the program point after it.
    ///
    /// The rules are the syntax-directed reachability fold: every
    /// construct that can end a path has one, and everything else folds
    /// over its children, so a divergence nested in a branch, a block, a
    /// match arm or a call argument is seen instead of being missed for
    /// sitting one level below the top of a statement.
    pub(super) fn expression_flow(&self, expression: &syn::Expr) -> Flow {
        match expression {
            syn::Expr::Return(returned) => {
                let value = returned
                    .expr
                    .as_deref()
                    .map_or_else(Flow::settled, |value| self.expression_flow(value));
                let mut flow = value.then(Flow::proving(Completion::Never));
                flow.returns = true;
                flow
            }
            syn::Expr::Break(broken) => {
                let value = broken
                    .expr
                    .as_deref()
                    .map_or_else(Flow::settled, |value| self.expression_flow(value));
                let mut flow = value.then(Flow::proving(Completion::Never));
                if let Some(label) = &broken.label {
                    flow.breaks.labeled.insert(label.ident.to_string());
                } else {
                    flow.breaks.bare = true;
                }
                flow
            }
            syn::Expr::Continue(continued) => {
                let mut flow = Flow::proving(Completion::Never);
                if let Some(label) = &continued.label {
                    flow.continues.labeled.insert(label.ident.to_string());
                } else {
                    flow.continues.bare = true;
                }
                flow
            }
            syn::Expr::Try(tried) => {
                let mut flow = self
                    .expression_flow(&tried.expr)
                    .then(Flow::proving(Completion::Unknown));
                flow.returns = true;
                flow
            }
            syn::Expr::Macro(invocation) => self.macro_flow(&invocation.mac),
            syn::Expr::Call(call) => {
                let flow = self.children_flow(expression);
                if self.process_exit(&call.func) || self.never_returns(expression) {
                    return flow.then(Flow::proving(Completion::Never));
                }
                if let syn::Expr::Closure(closure) = unwrapped(&call.func) {
                    return flow.then(self.invoked_body_flow(closure));
                }
                flow
            }
            syn::Expr::If(conditional) => self.conditional_flow(conditional),
            syn::Expr::Match(matched) => self.match_flow(matched),
            syn::Expr::Loop(repeated) => {
                let body = self.block_flow(&repeated.body);
                let escapes = body.breaks_out_of(repeated.label.as_ref()) || body.opaque_jumps;
                let mut flow = body.absorbed(repeated.label.as_ref());
                flow.completion = if escapes {
                    Completion::Unknown
                } else {
                    Completion::Never
                };
                flow
            }
            syn::Expr::While(conditional) => {
                let condition = self.expression_flow(&conditional.cond);
                let mut body = self
                    .block_flow(&conditional.body)
                    .absorbed(conditional.label.as_ref());
                body.completion = Completion::Always;
                condition.then(body)
            }
            syn::Expr::ForLoop(iterated) => {
                let iterable = self.expression_flow(&iterated.expr);
                let mut body = self
                    .block_flow(&iterated.body)
                    .absorbed(iterated.label.as_ref());
                body.completion = Completion::Always;
                iterable.then(body)
            }
            syn::Expr::Closure(_) | syn::Expr::Async(_) | syn::Expr::Const(_) => Flow::settled(),
            syn::Expr::Block(braced) => {
                let mut flow = self.block_flow(&braced.block);
                if let Some(label) = &braced.label
                    && flow.breaks.labeled.remove(&label.name.ident.to_string())
                {
                    flow.completion = flow.completion.join(Completion::Always);
                }
                flow
            }
            syn::Expr::Binary(binary)
                if matches!(binary.op, syn::BinOp::And(_) | syn::BinOp::Or(_)) =>
            {
                self.expression_flow(&binary.left)
                    .then(self.expression_flow(&binary.right).weakened())
            }
            syn::Expr::Verbatim(_) => Flow::opaque(),
            _ => self.children_flow(expression)
        }
    }
}
