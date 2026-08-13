// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Side-effect categories and allocations read off a written path, and
//! the effect an `unsafe` block marks the function with.

use rustmanifest_schema::Range;
use syn::spanned::Spanned as _;

use super::FactCollector;
use crate::semantic::{
    behavior::types::{Effect, EffectSite},
    knowledge
};

/// The module of the standard library a path runs through, and the
/// effect every call reaching that module performs.
///
/// A module, not a method: `write_all` on a file and `set_len` on the
/// same file are one boundary crossed, and which methods a library
/// declares there is the library's business. Forty-seven rows naming a
/// type and a method answered this for the instance calls, and answered
/// only for the forty-seven somebody wrote down.
const BOUNDARY_MODULES: [(&str, Effect); 4] = [
    ("fs", Effect::FileSystem),
    ("net", Effect::Network),
    ("process", Effect::Process),
    ("env", Effect::Environment)
];

/// The handles of the process's own console, whose module is the same
/// one every other reader and writer lives in.
const CONSOLE_HANDLES: [&str; 3] = ["Stdout", "Stderr", "Stdin"];

/// The effect a path performs, by the module of the library it runs
/// through and the handle it names.
pub(in crate::semantic::behavior::collector) fn reached_effect(
    segments: &[String]
) -> Option<Effect> {
    if segments.iter().any(|held| CONSOLE_HANDLES.contains(&held.as_str())) {
        return Some(Effect::ConsoleIo);
    }
    if let Some((_, effect)) = BOUNDARY_MODULES
        .iter()
        .find(|(module, _)| segments.iter().any(|held| held == module))
    {
        return Some(*effect);
    }
    (segments.len() == 2)
        .then(|| segments.first().and_then(|first| knowledge::crate_effect(first)))
        .flatten()
}

/// Crate the language declares its heap containers in, so a call the
/// compiler settles into it allocates.
const ALLOCATING_CRATE: &str = "alloc";

impl FactCollector {
    /// Whether the written path reaches the crate that allocates, as the
    /// compiler settled it.
    ///
    /// Five container names paired with three constructor names decided
    /// this before: a boxed value counted, a map or a deque did not, and
    /// a workspace type spelled `Vec` counted all the same.
    fn reaches_the_allocating_crate(&self, path: &syn::Path) -> bool {
        self.expanded_segments(path).first().map(String::as_str) == Some(ALLOCATING_CRATE)
    }

    /// Classifies a call path against the effect sink patterns, seen
    /// through the file's imports.
    pub(super) fn record_path_effect(&mut self, path: &syn::Path) {
        let segments = self.expanded_segments(path);
        let effect = reached_effect(&segments);
        if let Some(effect) = effect {
            self.facts.effects.push(effect);
            self.facts.effect_sites.push(EffectSite {
                effect,
                proof: segments.join("::"),
                range: span_range(path.span())
            });
        }
    }

    /// Records the effects and allocations a written path carries, and
    /// counts a bare name as one use of the parameter it may name.
    pub(super) fn collect_path(&mut self, path: &syn::ExprPath) {
        if path.path.segments.len() >= 2 {
            self.record_path_effect(&path.path);
            if self.reaches_the_allocating_crate(&path.path) {
                self.facts.allocations += 1;
            }
        } else if let Some(ident) = path.path.get_ident() {
            let name = ident.to_string();
            if self.imports.contains_key(&name) {
                self.record_path_effect(&path.path);
            }
            self.bump_param(&name, false);
        }
        syn::visit::visit_expr_path(self, path);
    }

    /// Records the database effect a SQL-carrying function call proves:
    /// the crate and function name say the argument is a statement, and
    /// the first written argument — where it is a string literal — says
    /// what kind. A statement built at runtime proves the call and only
    /// the call: the effect is recorded with its kind unproven, because
    /// a body that provably sends a statement must not read as pure —
    /// and the kind is not guessed from anything but written text.
    pub(super) fn record_sql_call(&mut self, call: &syn::ExprCall) {
        let syn::Expr::Path(path) = call.func.as_ref() else {
            return;
        };
        let segments = self.expanded_segments(&path.path);
        let Some(sql) = first_literal(call) else {
            if knowledge::sql_call(&segments) {
                self.facts.effects.push(Effect::Database);
                self.facts.effect_sites.push(EffectSite {
                    effect: Effect::Database,
                    proof:  segments.join("::"),
                    range:  span_range(path.span())
                });
            }
            return;
        };
        if let Some(effect) = knowledge::sql_call_effect(&segments, &sql) {
            self.facts.effects.push(effect);
            self.facts.effect_sites.push(EffectSite {
                effect,
                proof: segments.join("::"),
                range: span_range(path.span())
            });
            if let Some(statement) = knowledge::statement_of(&sql) {
                self.facts
                    .queries
                    .push(crate::semantic::behavior::query::QuerySite {
                        statement,
                        table: knowledge::table_of(&sql),
                        record: turbofish_row_type(&path.path),
                        binds: Vec::new(),
                        range: span_range(path.span())
                    });
            }
        }
    }

    /// Appends the plain name a `bind` call writes to the statement
    /// site its receiver chain stands on.
    ///
    /// The function form hands its arguments over one `bind` at a time,
    /// so the names stand outside the statement's own parentheses. Only
    /// a plain name is taken, a borrow seen through: a computed
    /// argument claims nothing, on the same terms as the macro's
    /// argument list. Runs after the chain below it was walked, so the
    /// statement site it appends to is already recorded and the names
    /// land in written order.
    pub(super) fn record_statement_bind(&mut self, call: &syn::ExprMethodCall) {
        if call.method != "bind" {
            return;
        }
        let Some(name) = plain_bound_name(call) else {
            return;
        };
        let Some(range) = statement_carrier(&call.receiver) else {
            return;
        };
        if let Some(site) = self
            .facts
            .queries
            .iter_mut()
            .find(|site| site.range == range)
        {
            site.binds.push(name);
        }
    }

    /// Marks the function as carrying the unsafe effect, citing the block
    /// itself as what proved it.
    pub(super) fn collect_unsafe_block(&mut self, block: &syn::ExprUnsafe) {
        self.facts.effects.push(Effect::Unsafe);
        self.facts.effect_sites.push(EffectSite {
            effect: Effect::Unsafe,
            proof:  "unsafe".to_owned(),
            range:  span_range(block.unsafe_token.span)
        });
        syn::visit::visit_expr_unsafe(self, block);
    }
}

/// The first argument of a call, where the source writes it as a string
/// literal.
fn first_literal(call: &syn::ExprCall) -> Option<String> {
    let syn::Expr::Lit(literal) = call.args.first()? else {
        return None;
    };
    let syn::Lit::Str(text) = &literal.lit else {
        return None;
    };
    Some(text.value())
}

/// Converts a span to the byte range the fact base records, saturating
/// rather than wrapping on a file larger than a `u32` can address.
fn span_range(span: proc_macro2::Span) -> Range {
    let bytes = span.byte_range();
    Range {
        start: u32::try_from(bytes.start).unwrap_or(u32::MAX),
        end:   u32::try_from(bytes.end).unwrap_or(u32::MAX)
    }
}

/// The row type a call's own turbofish writes, by its last segment.
///
/// The row-typing form takes the driver first and the row second, so
/// the last written type argument is the row. A call written without a
/// turbofish types its rows by inference, which is the compiler's
/// knowledge rather than the source's, and names none here.
fn turbofish_row_type(path: &syn::Path) -> Option<String> {
    let syn::PathArguments::AngleBracketed(written) = &path.segments.last()?.arguments else {
        return None;
    };
    written
        .args
        .iter()
        .filter_map(|argument| match argument {
            syn::GenericArgument::Type(syn::Type::Path(named)) => named
                .path
                .segments
                .last()
                .map(|segment| segment.ident.to_string()),
            _ => None
        })
        .next_back()
}

/// The one plain name a `bind` call is handed, a borrow seen through.
fn plain_bound_name(call: &syn::ExprMethodCall) -> Option<String> {
    let [only] = &call.args.iter().collect::<Vec<_>>()[..] else {
        return None;
    };
    let mut written = *only;
    if let syn::Expr::Reference(borrowed) = written {
        written = &borrowed.expr;
    }
    let syn::Expr::Path(path) = written else {
        return None;
    };
    path.path.get_ident().map(ToString::to_string)
}

/// The byte range of the statement-carrying call a method chain stands
/// on, awaits and propagation steps seen through.
fn statement_carrier(receiver: &syn::Expr) -> Option<Range> {
    let mut written = receiver;
    loop {
        match written {
            syn::Expr::MethodCall(chained) => written = &chained.receiver,
            syn::Expr::Await(awaited) => written = &awaited.base,
            syn::Expr::Try(tried) => written = &tried.expr,
            syn::Expr::Call(call) => {
                let syn::Expr::Path(path) = call.func.as_ref() else {
                    return None;
                };
                return Some(span_range(path.span()));
            }
            _ => return None
        }
    }
}
