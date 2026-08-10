// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The inlay-hint request: the ownership reading as ghost text at the
//! written arguments themselves.
//!
//! Every mark the engine derives for the document is offered inside
//! the requested range: `moves`, `copies`, `borrows`, `borrows mut`,
//! or `by value?` where `Copy`-ness is not provable, standing right
//! before the argument it describes — the fate of a value visible
//! while reading, with no hover asked.

use lsp_server::{Request, Response};
use lsp_types::{InlayHint, InlayHintLabel, InlayHintParams};

use super::{
    position::{LineIndex, offset_of},
    snapshot::{Derived, path_of}
};

/// Answers one inlay-hint request over the snapshot's derivations.
pub(super) fn answer(request: &Request, held: &Derived<'_>) -> Response {
    let Ok(params) = serde_json::from_value::<InlayHintParams>(request.params.clone()) else {
        return Response::new_err(
            request.id.clone(),
            lsp_server::ErrorCode::InvalidParams as i32,
            "malformed inlay hint parameters".to_owned()
        );
    };
    let path = path_of(params.text_document.uri.as_str());
    let hints: Vec<InlayHint> = held
        .sources
        .iter()
        .position(|source| source.path() == path)
        .map_or_else(Vec::new, |file| {
            let text = held.sources[file].text();
            let index = LineIndex::new(text);
            let from = offset_of(text, params.range.start);
            let until = offset_of(text, params.range.end);
            rustmanifest_engine::semantic::handover_marks(
                held.sources,
                held.facts,
                held.graph,
                held.types,
                file
            )
            .into_iter()
            .filter(|mark| {
                (mark.range.start as usize) >= from && (mark.range.end as usize) <= until
            })
            .map(|mark| InlayHint {
                position:      index.position(text, mark.range.start as usize),
                label:         InlayHintLabel::String(mark.label.to_owned()),
                kind:          None,
                text_edits:    None,
                tooltip:       None,
                padding_left:  Some(false),
                padding_right: Some(true),
                data:          None
            })
            .collect()
        });
    Response::new_ok(request.id.clone(), hints)
}
