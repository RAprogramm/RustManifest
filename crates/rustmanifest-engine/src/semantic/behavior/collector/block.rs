// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Walking the statements of a block, carrying along what each one proves
//! about the statement after it: a divergence kills what follows, an
//! unproven completion counts as one more exit standing over it.

use syn::visit::Visit;

use super::{FactCollector, completion::Completion, gate::gated};
use crate::semantic::behavior::output::Guard;

impl FactCollector {
    /// Visits the statements of a block in written order, updating the
    /// exits and the divergence standing over each of them.
    pub(super) fn collect_block_statements(&mut self, block: &syn::Block) {
        let entry_exits = self.prior_exits;
        let entry_dead = self.dead;
        for statement in &block.stmts {
            if gated(statement) {
                self.under(Guard::CfgGated, |collector| {
                    collector.visit_stmt(statement);
                });
            } else {
                self.visit_stmt(statement);
            }
            let flow = self.flow_scope().statement_flow(statement);
            match flow.completion {
                Completion::Never => self.dead = true,
                Completion::Unknown => self.prior_exits += 1,
                Completion::Always if flow.may_skip() => self.prior_exits += 1,
                Completion::Always => {}
            }
        }
        self.dead = entry_dead;
        self.prior_exits = entry_exits;
    }
}
