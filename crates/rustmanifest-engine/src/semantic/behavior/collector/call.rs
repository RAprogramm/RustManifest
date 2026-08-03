// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The call candidates this channel records: one normalized candidate per
//! call site, and the error conversion a `?` performs where the callee's
//! error type differs from the enclosing one.

use super::FactCollector;
use crate::semantic::behavior::types::CallSite;

impl FactCollector {
    /// Records one normalized call candidate, resolving `Self` and typed
    /// receivers through the local type map.
    pub(super) fn record_call(&mut self, owner: Option<&str>, callee: &str) {
        let candidate = match owner {
            None => callee.to_owned(),
            Some("Self") => self.param_types.get("self").map_or_else(
                || format!("Self::{callee}"),
                |own| format!("{own}::{callee}")
            ),
            Some(owner) => format!("{owner}::{callee}")
        };
        self.facts.call_sites.push(CallSite {
            candidate: candidate.clone(),
            guard:     self.site_guard()
        });
        self.facts.calls.push(candidate);
    }

    /// Records the error conversion a `?` performs where the callee's
    /// error type is not the one the enclosing signature declares.
    pub(super) fn collect_try(&mut self, tried: &syn::ExprTry) {
        if let [enclosing] = self.facts.error_types.clone().as_slice()
            && let Some(key) = self.call_key(&tried.expr)
            && let Some(callee_error) = self.ret(&key).and_then(|r| r.error.clone())
            && callee_error != *enclosing
        {
            self.record_call(Some(enclosing), "from");
        }
        syn::visit::visit_expr_try(self, tried);
    }
}
