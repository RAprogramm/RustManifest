// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The code-action request: fixes and the generating direction, both
//! offered in the editor's own action list.
//!
//! A cursor on a diagnostic offers the finding's own byte-precise
//! fixes as quickfixes — the same edits `check --fix` applies, every
//! edit of one finding carried together, a fix needing human review
//! saying so in its title. A cursor on a type's written name offers
//! one action per trait the facts can implement for it — every
//! workspace trait plus the std canon, each filtered through the same
//! skeleton synthesis the implement command uses, so a trait already
//! implemented, derived, or generic never reaches the list; the edit
//! appends the written impl block to the file that defines the type.
//! Everything is carried in the answer, so the client applies it
//! without asking the server anything further. A cursor anywhere else
//! answers an empty list, never an error.

use lsp_server::{Request, Response};
use lsp_types::{
    CodeAction, CodeActionKind, CodeActionParams, DocumentChanges, OneOf,
    OptionalVersionedTextDocumentIdentifier, Position, Range, TextDocumentEdit, TextEdit, Uri,
    WorkspaceEdit
};

use super::{
    analysis::Analysis,
    position::{LineIndex, offset_of},
    snapshot::{Derived, path_of}
};

/// Answers one code-action request over the snapshot's derivations.
pub(super) fn answer(request: &Request, held: &Derived<'_>, analysis: &Analysis) -> Response {
    let Ok(params) = serde_json::from_value::<CodeActionParams>(request.params.clone()) else {
        return Response::new_err(
            request.id.clone(),
            lsp_server::ErrorCode::InvalidParams as i32,
            "malformed code action parameters".to_owned()
        );
    };
    let mut offered = quickfixes(held, analysis, &params);
    if let Some(node) = named_type(held, &params) {
        offered.extend(implementations(held, node));
    }
    Response::new_ok(request.id.clone(), offered)
}

/// The quickfix actions of every finding overlapping the requested
/// range that carries byte-precise fixes.
fn quickfixes(
    held: &Derived<'_>,
    analysis: &Analysis,
    params: &CodeActionParams
) -> Vec<CodeAction> {
    let path = path_of(params.text_document.uri.as_str());
    let Some(file) = held.sources.iter().position(|source| source.path() == path) else {
        return Vec::new();
    };
    let text = held.sources[file].text();
    let index = LineIndex::new(text);
    let from = offset_of(text, params.range.start);
    let until = offset_of(text, params.range.end);
    let mut offered = Vec::new();
    for finding in analysis.findings(path.clone(), text) {
        let covered = finding.location.range.start as usize <= until
            && from <= finding.location.range.end as usize;
        if !covered || finding.fixes.is_empty() {
            continue;
        }
        if finding.fixes.iter().any(|fix| fix.rename_to.is_some()) {
            continue;
        }
        let edits: Vec<OneOf<TextEdit, lsp_types::AnnotatedTextEdit>> = finding
            .fixes
            .iter()
            .map(|fix| {
                OneOf::Left(TextEdit {
                    range:    Range {
                        start: index.position(text, fix.range.start as usize),
                        end:   index.position(text, fix.range.end as usize)
                    },
                    new_text: fix.replacement.clone()
                })
            })
            .collect();
        let review = finding
            .fixes
            .iter()
            .any(|fix| fix.applicability == rustmanifest_schema::Applicability::MaybeIncorrect);
        offered.push(CodeAction {
            title: format!(
                "fix {}{}",
                finding.rule_id,
                if review { " (needs review)" } else { "" }
            ),
            kind: Some(CodeActionKind::QUICKFIX),
            edit: Some(WorkspaceEdit {
                document_changes: Some(DocumentChanges::Edits(vec![TextDocumentEdit {
                    text_document: OptionalVersionedTextDocumentIdentifier {
                        uri:     params.text_document.uri.clone(),
                        version: None
                    },
                    edits
                }])),
                ..WorkspaceEdit::default()
            }),
            ..CodeAction::default()
        });
    }
    offered
}

/// The type whose written name the request's range touches, if any.
fn named_type<'a>(
    held: &'a Derived<'_>,
    params: &CodeActionParams
) -> Option<&'a rustmanifest_engine::TypeNode> {
    let path = path_of(params.text_document.uri.as_str());
    let file = held
        .sources
        .iter()
        .position(|source| source.path() == path)?;
    let text = held.sources[file].text();
    let offset = offset_of(text, params.range.start);
    held.types.types().iter().find(|node| {
        node.source == file
            && (node.range.start as usize..node.range.end as usize).contains(&offset)
    })
}

/// One action per trait the skeleton synthesis can write for `node`,
/// each carrying the edit that appends the impl to the defining file.
fn implementations(held: &Derived<'_>, node: &rustmanifest_engine::TypeNode) -> Vec<CodeAction> {
    let mut candidates: Vec<String> = held
        .types
        .traits()
        .iter()
        .map(|declared| declared.name.clone())
        .collect();
    for canon in ["Default", "Display"] {
        if !candidates.iter().any(|name| name == canon) {
            candidates.push(canon.to_owned());
        }
    }
    candidates.sort();
    let mut offered = Vec::new();
    for trait_name in candidates {
        let Some(written) = rustmanifest_engine::semantic::impl_skeleton(
            held.sources,
            held.types,
            &node.name,
            &trait_name
        ) else {
            continue;
        };
        let Some(edit) = appended(held, node.source, &written) else {
            continue;
        };
        offered.push(CodeAction {
            title: format!("implement {trait_name} for {}", node.name),
            kind: Some(CodeActionKind::REFACTOR),
            edit: Some(edit),
            ..CodeAction::default()
        });
    }
    offered
}

/// The workspace edit appending `written` to the end of source `file`.
fn appended(held: &Derived<'_>, file: usize, written: &str) -> Option<WorkspaceEdit> {
    let source = held.sources.get(file)?;
    let text = source.text();
    let uri: Uri = format!("file://{}", source.path().display()).parse().ok()?;
    let end: Position = LineIndex::new(text).position(text, text.len());
    let separated = if text.ends_with('\n') {
        format!("\n{written}\n")
    } else {
        format!("\n\n{written}\n")
    };
    let edited = TextDocumentEdit {
        text_document: OptionalVersionedTextDocumentIdentifier {
            uri,
            version: None
        },
        edits:         vec![OneOf::Left(TextEdit {
            range:    Range {
                start: end,
                end
            },
            new_text: separated
        })]
    };
    Some(WorkspaceEdit {
        document_changes: Some(DocumentChanges::Edits(vec![edited])),
        ..WorkspaceEdit::default()
    })
}
