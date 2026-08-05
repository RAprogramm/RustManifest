// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Walking a method call in the order its own values are settled in.

use super::{super::FactCollector, carrying::closure_input};
use crate::semantic::std_knowledge::Carried;

impl FactCollector {
    /// Walks a method call, binding a closure's own input to what the
    /// receiver holds before the closure's body is read.
    ///
    /// The receiver is walked first so that its own records exist when its
    /// origin is taken; only then does the binding stand, and only then is
    /// the body read. Walking the whole call at once would read the body
    /// while the closure's input still stood for nothing, and every fact
    /// recorded inside it would name a value the reading had not yet
    /// settled.
    pub(in crate::semantic::behavior::collector) fn walk_method_call(
        &mut self,
        call: &syn::ExprMethodCall
    ) {
        let method = call.method.to_string();
        let Some(Carried::ClosureOver(position)) =
            crate::semantic::std_knowledge::carried_by(&method)
        else {
            syn::visit::visit_expr_method_call(self, call);
            return;
        };
        syn::visit::visit_expr(self, &call.receiver);
        let held = self.origin_of(&call.receiver);
        let bound = call
            .args
            .iter()
            .nth(position.into())
            .and_then(|argument| match argument {
                syn::Expr::Closure(closure) => closure_input(closure),
                _ => None
            });
        let restored = bound
            .as_ref()
            .map(|name| (name.clone(), self.origins.insert(name.clone(), held)));
        for argument in &call.args {
            syn::visit::visit_expr(self, argument);
        }
        if let Some((name, previous)) = restored {
            match previous {
                Some(origin) => self.origins.insert(name, origin),
                None => self.origins.remove(&name)
            };
        }
    }
}
