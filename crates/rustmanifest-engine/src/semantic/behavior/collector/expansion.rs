// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a macro invocation contributes: the output it writes, the
//! allocation and the panic it carries, the effect its tokens name, and
//! the arguments the fact channel reads as expressions.

use syn::visit::Visit;

use super::{
    FactCollector,
    catalog::{EXPRESSION_MACROS, OUTPUT_MACROS, WRITER_MACROS}
};
use crate::semantic::{behavior::types::Effect, knowledge};

/// The crate the language declares its heap containers in, so a macro
/// the compiler settles into it allocates.
const ALLOCATING_CRATE: &str = "alloc";

/// The print macro that also names what it prints, which is why its
/// payload reads differently from the rest.
const DEBUG_PRINT: &str = "std::macros::dbg";

impl FactCollector {
    /// The item a macro invocation reaches, as the compiler settled the
    /// path it is written with.
    fn reached_macro(&self, invocation: &syn::Macro) -> Option<String> {
        use syn::spanned::Spanned;

        let span = invocation.path.span().byte_range();
        let (Ok(start), Ok(end)) = (u32::try_from(span.start), u32::try_from(span.end)) else {
            return None;
        };
        self.named.get(&(start, end)).cloned()
    }

    /// Records every fact one macro invocation carries.
    ///
    /// What the library's own macros do is read from the item each
    /// invocation reaches; what a dependency's macro carries — a
    /// statement, and the table it names — is the knowledge pack's own
    /// answer, keyed by the name that pack exports, and is read whether
    /// or not a resolver settled the path.
    pub(super) fn collect_macro(&mut self, invocation: &syn::Macro) {
        self.collect_statement_macro(invocation);
        if let Some(name) = self.reached_macro(invocation) {
            let name = name.as_str();
            if let Some((_, stream, newline)) = OUTPUT_MACROS
                .iter()
                .find(|(macro_name, _, _)| *macro_name == name)
            {
                self.facts.effects.push(Effect::ConsoleIo);
                self.record_output(invocation, *stream, *newline, name == DEBUG_PRINT);
            }
            if let Some((_, newline)) = WRITER_MACROS
                .iter()
                .find(|(macro_name, _)| *macro_name == name)
            {
                self.record_writer_output(invocation, *newline);
            }
            if name.split("::").next() == Some(ALLOCATING_CRATE) {
                self.facts.allocations += 1;
            }
            if self.leaves_from(invocation) {
                self.facts.flags = self.facts.flags.with_panic();
            }
            if EXPRESSION_MACROS.contains(&name) {
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

    /// Whether the compiler typed this macro's own position as one no
    /// value comes back from, which is what a panic-family macro does to
    /// the path it stands on.
    ///
    /// Four written macro names said this before, and said it for a
    /// program's own macro spelled the same way just as readily.
    fn leaves_from(&self, invocation: &syn::Macro) -> bool {
        use syn::spanned::Spanned;

        let span = invocation.span().byte_range();
        let (Ok(start), Ok(end)) = (u32::try_from(span.start), u32::try_from(span.end)) else {
            return false;
        };
        self.leaving.contains(&(start, end))
    }

    /// Records the statement a dependency's macro carries, where a
    /// knowledge pack names that macro.
    fn collect_statement_macro(&mut self, invocation: &syn::Macro) {
        let tokens = invocation.tokens.to_string();
        let spelled = invocation
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string())
            .unwrap_or_default();
        if let Some(effect) = knowledge::sql_macro_effect(&spelled, &tokens) {
            self.facts.effects.push(effect);
            self.record_macro_effect_site(invocation, effect);
            self.record_statement(invocation, &spelled, &tokens);
        }
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
