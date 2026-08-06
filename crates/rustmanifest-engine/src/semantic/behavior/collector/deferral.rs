// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The bodies that do not run where they are written: a closure, an
//! `async` block, and a nested item the block around it can call from
//! anywhere.

use super::FactCollector;
use crate::semantic::behavior::{
    output::Guard,
    value::{Condition, Provenance}
};

impl FactCollector {
    /// Visits a closure body under the frame a deferred body carries,
    /// with its own inputs standing for what whoever runs it hands in.
    ///
    /// An input no nearer rule answered is supplied from outside this
    /// body — a settled answer and not a refusal, exactly as a
    /// parameter's value is settled outside the function declaring it.
    /// The binding lives as long as the body it belongs to and no
    /// longer.
    pub(super) fn collect_closure(&mut self, closure: &syn::ExprClosure) {
        self.under(Guard::Closure, Condition::NotDerived, |collector| {
            collector.scoped(|collector| {
                for input in &closure.inputs {
                    let Some(named) = super::naming::binding_name(input) else {
                        continue;
                    };
                    if collector.origins.contains_key(&named) {
                        continue;
                    }
                    collector
                        .origins
                        .insert(named.clone(), Provenance::Supplied(named));
                }
                syn::visit::visit_expr_closure(collector, closure);
            });
        });
    }

    /// An `async` block builds a future whose body runs where the future
    /// is driven, not where it is written — the same shape a closure
    /// has, and it carries the same frame. Without one, a write in a
    /// future that is never polled was graded as running on every path.
    pub(super) fn collect_async_block(&mut self, block: &syn::ExprAsync) {
        self.under(Guard::Closure, Condition::NotDerived, |collector| {
            syn::visit::visit_expr_async(collector, block);
        });
    }

    /// A nested item is order-independent: it is visible to the whole
    /// block around it and is callable from before the line it is
    /// written on. What stands over that line therefore says nothing
    /// about it, so the enclosing divergence and the exits counted
    /// before it are both dropped on the way in and restored on the way
    /// out.
    pub(super) fn collect_nested_fn(&mut self, nested: &syn::ItemFn) {
        let entry_dead = self.dead;
        let entry_exits = self.prior_exits;
        self.dead = false;
        self.prior_exits = 0;
        self.under(Guard::NestedItem, Condition::NotDerived, |collector| {
            syn::visit::visit_item_fn(collector, nested);
        });
        self.dead = entry_dead;
        self.prior_exits = entry_exits;
    }
}
