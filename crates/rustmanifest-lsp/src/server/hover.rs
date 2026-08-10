// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The hover request: the fact card of the function under the cursor,
//! computed over the whole workspace snapshot.
//!
//! The cursor names a function by standing on its written name — the
//! one byte range the fact base records per function. A cursor on a
//! written call instead answers with the ownership story of that call:
//! per handed value, whether it moves, crosses as a copy, or is only
//! borrowed. A cursor anywhere else answers with nothing, which the
//! protocol renders as no card at all: silence is "no fact recorded
//! here", never an empty box. The facts and the inference read every
//! source the snapshot holds, so a chain crossing files answers the
//! same as one inside a single buffer.

use lsp_server::{Request, Response};
use lsp_types::{Hover, HoverContents, HoverParams, MarkupContent, MarkupKind};

use super::{
    card::{card, handover_section, type_card, why_section},
    position::offset_of,
    snapshot::{Derived, path_of}
};

/// Answers one hover request over the snapshot's derivations.
pub(super) fn answer(request: &Request, held: &Derived<'_>) -> Response {
    let Ok(params) = serde_json::from_value::<HoverParams>(request.params.clone()) else {
        return Response::new_err(
            request.id.clone(),
            lsp_server::ErrorCode::InvalidParams as i32,
            "malformed hover parameters".to_owned()
        );
    };
    let uri = &params.text_document_position_params.text_document.uri;
    let path = path_of(uri.as_str());
    let answered = held
        .sources
        .iter()
        .position(|source| source.path() == path)
        .and_then(|file| {
            let text = held.sources[file].text();
            let offset = offset_of(text, params.text_document_position_params.position);
            named_card(held, file, offset)
                .or_else(|| typed_card(held, file, offset))
                .or_else(|| call_story(held, file, offset))
        })
        .map(|value| Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value
            }),
            range:    None
        });
    answered.map_or_else(
        || Response::new_ok(request.id.clone(), serde_json::Value::Null),
        |hover| Response::new_ok(request.id.clone(), hover)
    )
}

/// The fact card of the function whose written name covers `offset`,
/// with the inference of every effect it only inherits.
fn named_card(held: &Derived<'_>, file: usize, offset: usize) -> Option<String> {
    let (index, fact) = held.facts.iter().enumerate().find(|(_, fact)| {
        let range = fact.range;
        fact.source == file && (range.start as usize..range.end as usize).contains(&offset)
    })?;
    let inherited = held.reached.get(index).map_or(&[][..], Vec::as_slice);
    let mut value = card(fact, inherited);
    for effect in inherited
        .iter()
        .filter(|effect| !fact.effects.contains(effect))
    {
        let named = effect.to_string();
        let asked = rustmanifest_engine::semantic::why_effect_over(
            held.sources,
            held.facts,
            held.graph,
            &fact.function,
            &named
        );
        value.push_str("\n\n");
        value.push_str(&why_section(&named, &asked));
    }
    Some(value)
}

/// The fact card of the type whose written name covers `offset`.
fn typed_card(held: &Derived<'_>, file: usize, offset: usize) -> Option<String> {
    let node = held.types.types().iter().find(|node| {
        node.source == file
            && (node.range.start as usize..node.range.end as usize).contains(&offset)
    })?;
    let implemented = held.types.traits_implemented(&node.name);
    let shape = held.shapes.iter().find(|shape| shape.name == node.name);
    Some(type_card(node, &implemented, shape))
}

/// The ownership story of the written call covering `offset`, where a
/// resolved workspace call with parameters stands there.
fn call_story(held: &Derived<'_>, file: usize, offset: usize) -> Option<String> {
    let answer = rustmanifest_engine::semantic::handover_at(
        held.facts,
        held.graph,
        held.types,
        file,
        u32::try_from(offset).ok()?
    )?;
    Some(handover_section(&answer))
}
