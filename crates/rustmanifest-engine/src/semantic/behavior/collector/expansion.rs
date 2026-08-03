// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a macro invocation contributes: the output it writes, the
//! allocation and the panic it carries, the effect its tokens name, and
//! the arguments the fact channel reads as expressions.

use syn::visit::Visit;

use super::{
    FactCollector,
    catalog::{DIVERGING_MACROS, EXPRESSION_MACROS, OUTPUT_MACROS, WRITER_MACROS}
};
use crate::semantic::{behavior::types::Effect, knowledge};

impl FactCollector {
    /// Records every fact one macro invocation carries.
    pub(super) fn collect_macro(&mut self, invocation: &syn::Macro) {
        if let Some(segment) = invocation.path.segments.last() {
            let name = segment.ident.to_string();
            if let Some((_, stream, newline)) = OUTPUT_MACROS
                .iter()
                .find(|(macro_name, _, _)| *macro_name == name)
            {
                self.facts.effects.push(Effect::ConsoleIo);
                self.record_output(invocation, *stream, *newline, name == "dbg");
            }
            if let Some((_, newline)) = WRITER_MACROS
                .iter()
                .find(|(macro_name, _)| *macro_name == name)
            {
                self.record_writer_output(invocation, *newline);
            }
            if name == "vec" || name == "format" {
                self.facts.allocations += 1;
            }
            if DIVERGING_MACROS.contains(&name.as_str()) {
                self.facts.flags = self.facts.flags.with_panic();
            }
            if let Some(effect) =
                knowledge::sql_macro_effect(&name, &invocation.tokens.to_string())
            {
                self.facts.effects.push(effect);
            }
            if EXPRESSION_MACROS.contains(&name.as_str()) {
                self.visit_macro_arguments(invocation);
            }
        }
        syn::visit::visit_macro(self, invocation);
    }

    /// Visits the arguments of a macro whose arguments are expressions,
    /// so a fact standing inside one is recorded rather than dropped for
    /// the depth it sits at. The reachability fold already reads these
    /// same tokens; reading them for one channel and not the other is
    /// what let a write inside a macro argument disappear.
    fn visit_macro_arguments(&mut self, invocation: &syn::Macro) {
        let Ok(arguments) = invocation.parse_body_with(
            syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated
        ) else {
            return;
        };
        for argument in &arguments {
            Visit::visit_expr(self, argument);
        }
    }
}
