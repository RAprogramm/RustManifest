// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The call candidates this channel records: one normalized candidate per
//! call site, what the site hands over, where it stands, and the error
//! conversion a `?` performs where the callee's error type differs from
//! the enclosing one.

use rustmanifest_schema::Range;

use super::FactCollector;
use crate::semantic::behavior::{
    types::CallSite,
    value::{Arguments, Disposal}
};

impl FactCollector {
    /// Records one call candidate whose arguments the source writes
    /// nowhere to be read.
    ///
    /// Three sites are of that kind: the error conversion behind a `?`,
    /// the poll an `await` stands for, and the inner call a service
    /// wrapper dispatches to. None of them is written as a call, so an
    /// empty argument list would read as a call of no arguments, which is
    /// a different fact.
    pub(super) fn record_call(&mut self, owner: Option<&str>, callee: &str) {
        self.push_call(owner, callee, None, Arguments::Unwritten);
    }

    /// Records one call the source writes, with the origin of every
    /// argument it hands over and the byte range of the calling
    /// construct.
    pub(super) fn record_written_call(
        &mut self,
        owner: Option<&str>,
        callee: &str,
        span: &std::ops::Range<usize>,
        arguments: &[&syn::Expr]
    ) {
        let origins = arguments
            .iter()
            .map(|argument| self.origin_of(argument))
            .collect();
        self.push_call(owner, callee, Some(span), Arguments::Written(origins));
    }

    /// Pushes one call site and its candidate.
    fn push_call(
        &mut self,
        owner: Option<&str>,
        callee: &str,
        span: Option<&std::ops::Range<usize>>,
        arguments: Arguments
    ) {
        let candidate = match owner {
            None => callee.to_owned(),
            Some("Self") => self.param_types.get("self").map_or_else(
                || format!("Self::{callee}"),
                |own| format!("{own}::{callee}")
            ),
            Some(owner) => format!("{owner}::{callee}")
        };
        let site = span.map_or(
            Range {
                start: 0, end: 0
            },
            |written| Range {
                start: u32::try_from(written.start).unwrap_or(u32::MAX),
                end:   u32::try_from(written.end).unwrap_or(u32::MAX)
            }
        );
        self.facts.call_sites.push(CallSite {
            candidate: candidate.clone(),
            guard: self.site_guard(),
            site,
            arguments,
            disposal: Disposal::NotIdentified
        });
        self.facts.calls.push(candidate);
    }

    /// Records the error conversion a `?` performs where the callee's
    /// error type is not the one the enclosing signature declares.
    pub(super) fn collect_try(&mut self, tried: &syn::ExprTry) {
        self.record_try_handback(tried);
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
