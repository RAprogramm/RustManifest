// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Intent diagnostics: the workspace's own written calls, published
//! where they stand.
//!
//! Each derived intent whose citing call is written in the published
//! document lands as a diagnostic on the call site itself, and its
//! related information leads to the declaration of the type the intent
//! is about — the surface where the implement action is offered. The
//! citation is thereby a place the editor jumps to from either end,
//! not a title to read. The set refreshes with the snapshot: on open
//! and on save, when the workspace is re-derived anyway; a keystroke
//! republishes only the per-document findings, so typing never pays
//! for a workspace walk.

use lsp_types::{
    Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity, Location, NumberOrString, Range
};

use super::{position::LineIndex, snapshot::Derived};

/// The intent diagnostics of one published document: every derived
/// intent whose citing call it holds, standing at the call site.
pub(super) fn diagnostics(held: &Derived<'_>, path: &std::path::Path) -> Vec<Diagnostic> {
    let Some(file) = held.sources.iter().position(|source| source.path() == path) else {
        return Vec::new();
    };
    let text = held.sources[file].text();
    let index = LineIndex::new(text);
    let mut found = Vec::new();
    for intent in rustmanifest_engine::semantic::derived_intents_with(held.facts, held.types) {
        if intent.source != file {
            continue;
        }
        found.push(Diagnostic {
            range: Range {
                start: index.position(text, intent.site.start as usize),
                end:   index.position(text, intent.site.end as usize)
            },
            severity: Some(DiagnosticSeverity::INFORMATION),
            code: Some(NumberOrString::String("intent".to_owned())),
            source: Some("rust-manifest".to_owned()),
            message: intent.evidence.fact.clone(),
            related_information: declared(held, &intent),
            ..Diagnostic::default()
        });
    }
    found
}

/// The declaration the intent's related information leads to: the
/// cited type's own written name, where the implement action stands.
fn declared(
    held: &Derived<'_>,
    intent: &rustmanifest_engine::semantic::DerivedIntent
) -> Option<Vec<DiagnosticRelatedInformation>> {
    let node = held.types.type_named(&intent.type_name)?;
    let source = held.sources.get(node.source)?;
    let text = source.text();
    let index = LineIndex::new(text);
    Some(vec![DiagnosticRelatedInformation {
        location: Location {
            uri:   format!("file://{}", source.path().display()).parse().ok()?,
            range: Range {
                start: index.position(text, node.range.start as usize),
                end:   index.position(text, node.range.end as usize)
            }
        },
        message:  format!(
            "`{}` is declared here, without `{}` — the implement action is offered on its name",
            intent.type_name, intent.trait_name
        )
    }])
}
