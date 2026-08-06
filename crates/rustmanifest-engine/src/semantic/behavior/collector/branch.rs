// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The branching constructs as the fact channel walks them: each branch
//! carries its own frame, and the one arm of a match that binds rather
//! than branches carries none.

use syn::visit::Visit;

use super::{
    FactCollector,
    pattern::{arm_condition, irrefutable}
};
use crate::semantic::behavior::{output::Guard, value::Condition};

impl FactCollector {
    /// Visits a conditional, each branch under its own frame.
    pub(super) fn collect_conditional(&mut self, conditional: &syn::ExprIf) {
        self.visit_expr(&conditional.cond);
        let test = self.origin_of(&conditional.cond);
        self.under(
            Guard::If,
            Condition::Branch {
                test:  test.clone(),
                taken: true
            },
            |collector| {
                collector.scoped(|collector| {
                    collector.visit_block(&conditional.then_branch);
                });
            }
        );
        if let Some((_, otherwise)) = &conditional.else_branch {
            self.under(
                Guard::Else,
                Condition::Branch {
                    test,
                    taken: false
                },
                |collector| {
                    collector.scoped(|collector| {
                        collector.visit_expr(otherwise);
                    });
                }
            );
        }
    }

    /// Visits a match, each arm under the frame its pattern earns.
    pub(super) fn collect_match(&mut self, matched: &syn::ExprMatch) {
        use syn::spanned::Spanned;

        self.visit_expr(&matched.expr);
        self.record_outcome_bindings(matched);
        let span = matched.span().byte_range();
        let scrutinee = self.origin_of(&matched.expr);
        let single = matched.arms.len() == 1;
        let choice = self.choices;
        self.choices += 1;
        let record = self.reserve_choice(&span, scrutinee);
        for (position, arm) in matched.arms.iter().enumerate() {
            let frame = Guard::MatchArm {
                irrefutable: irrefutable(&arm.pat),
                guarded:     arm_condition(&arm.pat).is_some()
            };
            let binds = single
                && matches!(
                    frame,
                    Guard::MatchArm {
                        irrefutable: true,
                        guarded:     false
                    }
                );
            if binds {
                self.visit_arm(arm);
                self.record_alternative(record, arm);
            } else {
                let alternative = u32::try_from(position).unwrap_or(u32::MAX);
                self.under(
                    frame,
                    Condition::Arm {
                        choice,
                        alternative
                    },
                    |collector| {
                        collector.scoped(|collector| {
                            collector.visit_arm(arm);
                            collector.record_alternative(record, arm);
                        });
                    }
                );
            }
        }
        self.settle_choice(record, &span);
    }
}
