// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The ownership marks of one source: a short label per written
//! argument, for surfaces that annotate the text itself rather than
//! answer a question about one position.
//!
//! Each mark repeats the handover reading in one word: `moves`,
//! `copies`, `borrows`, `borrows mut`, or `by value?` where `Copy`-ness
//! is not provable. Marks exist only where the reading holds whole: the
//! callee resolves to exactly one workspace function and the written
//! arguments align with its declared parameters, receiver included.
//! Anything else gets no mark — an absent label is a smaller claim.

use rustmanifest_schema::Range;
use syn::visit::Visit;

use super::{
    super::{
        behavior::{FunctionFacts, ParamMode},
        calls::CallGraph,
        graph::TypeGraph
    },
    copyness, resolved_callee
};
use crate::source::Source;

/// One ownership label at the written argument it describes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mark {
    /// Byte range of the written argument.
    pub range: Range,
    /// The reading, in one word.
    pub label: &'static str
}

/// The ownership marks of source `file`, in text order.
#[must_use]
pub fn handover_marks(
    sources: &[Source],
    facts: &[FunctionFacts],
    graph: &CallGraph,
    types: &TypeGraph,
    settled: Option<&rustmanifest_schema::Resolution>,
    file: usize
) -> Vec<Mark> {
    let Some(source) = sources.get(file) else {
        return Vec::new();
    };
    let Ok(parsed) = syn::parse_file(source.text()) else {
        return Vec::new();
    };
    let mut written = Written {
        calls: Vec::new()
    };
    written.visit_file(&parsed);
    let mut marks = Vec::new();
    for (caller, fact) in facts.iter().enumerate() {
        if fact.source != file {
            continue;
        }
        for site in &fact.call_sites {
            let Some(callee) = resolved_callee(facts, graph, caller, site) else {
                continue;
            };
            let Some(call) = written.calls.iter().find(|call| call.site == site.site) else {
                continue;
            };
            marks.extend(aligned(&facts[callee], call, types, settled));
        }
    }
    marks.sort_by_key(|mark| mark.range.start);
    marks
}

/// The marks of one call whose written arguments align with the
/// callee's declared parameters, receiver included.
fn aligned(
    callee: &FunctionFacts,
    call: &Call,
    types: &TypeGraph,
    settled: Option<&rustmanifest_schema::Resolution>
) -> Vec<Mark> {
    let params = &callee.params;
    let skipped = if params.len() == call.arguments.len() {
        0
    } else if params.len() == call.arguments.len() + 1 && call.method {
        1
    } else {
        return Vec::new();
    };
    params[skipped..]
        .iter()
        .zip(&call.arguments)
        .map(|(param, argument)| Mark {
            range: *argument,
            label: match param.mode {
                ParamMode::Owned => match copyness(types, settled, &param.type_name) {
                    Some(false) => "moves",
                    Some(true) => "copies",
                    None => "by value?"
                },
                ParamMode::Borrowed => "borrows",
                ParamMode::BorrowedMut => "borrows mut"
            }
        })
        .collect()
}

/// One written call: where it stands and where its arguments do.
struct Call {
    /// Byte range of the whole calling construct.
    site:      Range,
    /// Byte ranges of the written arguments, receiver excluded.
    arguments: Vec<Range>,
    /// Whether the call has a receiver.
    method:    bool
}

/// Collects every written call with its argument spans.
struct Written {
    calls: Vec<Call>
}

impl<'ast> Visit<'ast> for Written {
    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        self.calls.push(Call {
            site:      ranged(syn::spanned::Spanned::span(call)),
            arguments: call
                .args
                .iter()
                .map(|argument| ranged(syn::spanned::Spanned::span(argument)))
                .collect(),
            method:    false
        });
        syn::visit::visit_expr_call(self, call);
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        self.calls.push(Call {
            site:      ranged(syn::spanned::Spanned::span(call)),
            arguments: call
                .args
                .iter()
                .map(|argument| ranged(syn::spanned::Spanned::span(argument)))
                .collect(),
            method:    true
        });
        syn::visit::visit_expr_method_call(self, call);
    }
}

/// The schema range of one span.
fn ranged(span: proc_macro2::Span) -> Range {
    let bytes = span.byte_range();
    Range {
        start: u32::try_from(bytes.start).unwrap_or(u32::MAX),
        end:   u32::try_from(bytes.end).unwrap_or(u32::MAX)
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use std::path::PathBuf;

    use super::handover_marks;
    use crate::{
        semantic::{
            calls::{call_graph, resolution_context_with},
            graph::TypeGraph,
            settling::settle_identity
        },
        source::Source
    };

    fn marked(text: &str) -> Vec<(String, &'static str)> {
        let sources = [Source::new(PathBuf::from("src/lib.rs"), text.to_owned())];
        let facts = crate::semantic::behavior_with(
            &sources,
            Some(&crate::semantic::settling::settled_over(&sources))
        );
        let types = TypeGraph::from_sources(&sources);
        let mut resolution = rustmanifest_schema::Resolution {
            resolver: "test".to_owned(),
            types_granted: vec![rustmanifest_schema::ResolvedProperty {
                named: "String".to_owned(),
                copy:  false
            }],
            ..rustmanifest_schema::Resolution::default()
        };
        settle_identity(&mut resolution, &sources, &facts);
        let graph = call_graph(
            &facts,
            &resolution_context_with(&sources, Some(&resolution))
        );
        handover_marks(&sources, &facts, &graph, &types, Some(&resolution), 0)
            .into_iter()
            .map(|mark| {
                (
                    text[mark.range.start as usize..mark.range.end as usize].to_owned(),
                    mark.label
                )
            })
            .collect()
    }

    #[test]
    fn an_unresolved_callee_marks_nothing() {
        assert!(marked("fn main() { let s = String::from(\"x\"); s.len(); }").is_empty());
    }
}
