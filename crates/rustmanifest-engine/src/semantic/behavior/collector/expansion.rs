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
            let tokens = invocation.tokens.to_string();
            if let Some(effect) = knowledge::sql_macro_effect(&name, &tokens) {
                self.facts.effects.push(effect);
                self.record_macro_effect_site(invocation, effect);
                self.record_statement(invocation, &name, &tokens);
            }
            if EXPRESSION_MACROS.contains(&name.as_str()) {
                self.visit_macro_arguments(invocation);
            }
            if OUTPUT_MACROS
                .iter()
                .any(|(macro_name, _, _)| *macro_name == name)
            {
                self.settle_hole_origins(invocation, 1);
            }
            if WRITER_MACROS
                .iter()
                .any(|(macro_name, _)| *macro_name == name)
            {
                self.settle_hole_origins(invocation, 2);
            }
        }
        syn::visit::visit_macro(self, invocation);
    }

    /// Records the site an effect-carrying macro proved its effect at:
    /// the macro path as the source spells it, over the path's own byte
    /// range, so the effect carries a citation like every call-proven
    /// effect does.
    fn record_macro_effect_site(&mut self, invocation: &syn::Macro, effect: Effect) {
        use syn::spanned::Spanned;
        let proof = invocation
            .path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect::<Vec<_>>()
            .join("::");
        let bytes = invocation.path.span().byte_range();
        self.facts
            .effect_sites
            .push(crate::semantic::behavior::types::EffectSite {
                effect,
                proof,
                range: rustmanifest_schema::Range {
                    start: u32::try_from(bytes.start).unwrap_or(u32::MAX),
                    end:   u32::try_from(bytes.end).unwrap_or(u32::MAX)
                }
            });
    }

    /// Records the written statement a SQL-carrying macro proves: the
    /// kind from its first keyword, the table from the grammar slot
    /// where a single plain name fills it, the row type where the
    /// row-typing macro writes one ahead of the SQL, and the plain
    /// names written as arguments after it — all over the macro path's
    /// own range.
    fn record_statement(&mut self, invocation: &syn::Macro, name: &str, tokens: &str) {
        use syn::spanned::Spanned;
        let mut parts = tokens.splitn(3, '"');
        let (Some(head), Some(sql), tail) = (parts.next(), parts.next(), parts.next()) else {
            return;
        };
        let Some(statement) = knowledge::statement_of(sql) else {
            return;
        };
        let record = name
            .starts_with("query_as")
            .then(|| plain_written_names(head).into_iter().next())
            .flatten();
        let bytes = invocation.path.span().byte_range();
        self.facts
            .queries
            .push(crate::semantic::behavior::query::QuerySite {
                statement,
                table: knowledge::table_of(sql),
                record,
                binds: tail.map(plain_written_names).unwrap_or_default(),
                range: rustmanifest_schema::Range {
                    start: u32::try_from(bytes.start).unwrap_or(u32::MAX),
                    end:   u32::try_from(bytes.end).unwrap_or(u32::MAX)
                }
            });
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

/// The plain names written among comma-separated macro tokens, paths
/// kept by their last segment.
///
/// A part that is anything other than one written name — a field
/// access, a call, a literal — is not listed: what a computed argument
/// holds is another tier's question, and this reading records only
/// what the site spells.
fn plain_written_names(text: &str) -> Vec<String> {
    text.split(',')
        .filter_map(|part| {
            let last = part.split("::").last()?.trim();
            let named = !last.is_empty()
                && !last.starts_with(|first: char| first.is_ascii_digit())
                && last
                    .chars()
                    .all(|piece| piece.is_ascii_alphanumeric() || piece == '_');
            named.then(|| last.to_owned())
        })
        .collect()
}
