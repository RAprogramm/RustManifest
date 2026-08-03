// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The branching constructs of the reachability fold: what a conditional
//! and a match prove once their arms are merged, the sequential fold over
//! the children of everything else, and the body of a closure invoked
//! where it is written.

use syn::visit::Visit;

use super::{completion::Completion, flow::Flow, pattern::arm_condition, scope::FlowScope};

impl FlowScope<'_> {
    /// What a conditional proves: the branch not taken is as real as the
    /// one taken, and a missing `else` branch is a branch that
    /// completes.
    pub(super) fn conditional_flow(&self, conditional: &syn::ExprIf) -> Flow {
        let condition = self.expression_flow(&conditional.cond);
        let taken = self.block_flow(&conditional.then_branch);
        let otherwise = conditional
            .else_branch
            .as_ref()
            .map_or_else(Flow::settled, |(_, branch)| self.expression_flow(branch));
        condition.then(taken.join(otherwise))
    }

    /// What a match proves: control enters exactly one arm, so the arms
    /// merge. A match with no arms is matching a value that cannot exist
    /// and never completes.
    pub(super) fn match_flow(&self, matched: &syn::ExprMatch) -> Flow {
        let scrutinee = self.expression_flow(&matched.expr);
        let mut arms: Option<Flow> = None;
        for arm in &matched.arms {
            let condition = arm_condition(&arm.pat).map_or_else(Flow::settled, |guard| {
                self.expression_flow(guard).weakened()
            });
            let taken = condition.then(self.expression_flow(&arm.body));
            arms = Some(match arms {
                Some(seen) => seen.join(taken),
                None => taken
            });
        }
        scrutinee.then(arms.unwrap_or_else(|| Flow::proving(Completion::Never)))
    }

    /// Sequential fold over the immediate children of a construct whose
    /// own control flow needs no rule of its own — every child runs, in
    /// written order. Nested items are not children of the flow: their
    /// bodies run where they are called, not here.
    pub(super) fn children_flow(&self, expression: &syn::Expr) -> Flow {
        struct Children<'scope, 'imports> {
            flow:  Flow,
            scope: &'scope FlowScope<'imports>
        }
        impl<'ast> Visit<'ast> for Children<'_, '_> {
            fn visit_expr(&mut self, child: &'ast syn::Expr) {
                self.flow = std::mem::take(&mut self.flow).then(self.scope.expression_flow(child));
            }

            fn visit_block(&mut self, child: &'ast syn::Block) {
                self.flow = std::mem::take(&mut self.flow).then(self.scope.block_flow(child));
            }

            fn visit_item(&mut self, _item: &'ast syn::Item) {}
        }
        let mut children = Children {
            flow:  Flow::settled(),
            scope: self
        };
        syn::visit::visit_expr(&mut children, expression);
        children.flow
    }

    /// What a closure invoked where it is written proves: its body runs
    /// here, so a divergence in it ends this path. A `return` inside
    /// leaves the closure and not the function around it, which no
    /// syntax here tells apart from a body that falls off its end, so a
    /// returning body proves nothing either way.
    pub(super) fn invoked_body_flow(&self, closure: &syn::ExprClosure) -> Flow {
        let body = self.expression_flow(&closure.body);
        let completion = if body.returns {
            body.completion.join(Completion::Always)
        } else {
            body.completion
        };
        Flow {
            completion,
            opaque_jumps: body.opaque_jumps,
            ..Flow::default()
        }
    }
}
