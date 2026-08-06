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
    ///
    /// A conditional with an else chooses between two written
    /// alternatives exactly as a match does, so it is recorded as one
    /// choice: the condition is what it chooses on, the else accepts
    /// every remaining value, and the value of the whole is whichever
    /// branch runs. Without an else there is no second written
    /// alternative, and the construct is left as the statement it is.
    pub(super) fn collect_conditional(&mut self, conditional: &syn::ExprIf) {
        use syn::spanned::Spanned;

        self.visit_expr(&conditional.cond);
        let test = self.origin_of(&conditional.cond);
        let span = conditional.span().byte_range();
        let record = conditional
            .else_branch
            .is_some()
            .then(|| self.reserve_choice(&span, test.clone()));
        self.under(
            Guard::If,
            Condition::Branch {
                test:  test.clone(),
                taken: true
            },
            |collector| {
                collector.scoped(|collector| {
                    collector.visit_block(&conditional.then_branch);
                    if let Some(choice) = record {
                        let taken = collector.block_value(&conditional.then_branch);
                        collector.record_branch(choice, taken, false);
                    }
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
                        if let Some(choice) = record {
                            let left = collector.origin_of(otherwise);
                            collector.record_branch(choice, left, true);
                        }
                    });
                }
            );
        }
        if let Some(choice) = record {
            self.settle_choice(choice, &span);
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
