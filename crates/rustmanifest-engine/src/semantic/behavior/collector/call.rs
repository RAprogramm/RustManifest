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
    value::{Arguments, Disposal, Provenance}
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

    /// Reserves the record of one call the source writes, answering with
    /// its position in the channel.
    ///
    /// The record is pushed before the call's own operands are walked, so
    /// the channel keeps body order and an expression written around this
    /// call can already name it. What the site hands over is filled in
    /// afterwards by [`Self::settle_arguments`], once the operands have
    /// records of their own to be named by — reading them first would
    /// name a value the walk had not reached yet.
    pub(super) fn reserve_written_call(
        &mut self,
        owner: Option<&str>,
        callee: &str,
        span: &std::ops::Range<usize>
    ) -> usize {
        let index = self.facts.call_sites.len();
        self.push_call(owner, callee, Some(span), Arguments::Written(Vec::new()));
        let produced = rustmanifest_schema::Range {
            start: u32::try_from(span.start).unwrap_or(u32::MAX),
            end:   u32::try_from(span.end).unwrap_or(u32::MAX)
        };
        self.produced.insert(
            (produced.start, produced.end),
            Provenance::Called(u32::try_from(index).unwrap_or(u32::MAX))
        );
        index
    }

    /// Fills in what a reserved call site hands over.
    pub(super) fn settle_arguments(&mut self, index: usize, arguments: &[&syn::Expr]) {
        let origins: Vec<Provenance> = arguments
            .iter()
            .map(|argument| self.origin_of(argument))
            .collect();
        if let Some(site) = self.facts.call_sites.get_mut(index) {
            site.arguments = Arguments::Written(origins);
        }
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
        syn::visit::visit_expr_try(self, tried);
        self.record_try_handback(tried);
        if let [enclosing] = self.facts.error_types.clone().as_slice()
            && let Some(key) = self.call_key(&tried.expr)
            && let Some(callee_error) = self.ret(&key).and_then(|r| r.error.clone())
            && callee_error != *enclosing
        {
            self.record_call(Some(enclosing), "from");
        }
    }
}
