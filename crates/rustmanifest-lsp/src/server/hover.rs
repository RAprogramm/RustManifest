// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The hover request: the fact card of the function under the cursor.
//!
//! The cursor names a function by standing on its written name — the
//! one byte range the fact base records per function. A cursor
//! anywhere else answers with nothing, which the protocol renders as
//! no card at all: silence is "no fact recorded here", never an empty
//! box.

use lsp_server::{Request, Response};
use lsp_types::{Hover, HoverContents, HoverParams, MarkupContent, MarkupKind};

use super::{
    card::{card, why_section},
    position::offset_of
};

/// Answers one hover request from the stored text of the document.
pub(super) fn answer(request: &Request, text: Option<&str>) -> Response {
    let Ok(params) = serde_json::from_value::<HoverParams>(request.params.clone()) else {
        return Response::new_err(
            request.id.clone(),
            lsp_server::ErrorCode::InvalidParams as i32,
            "malformed hover parameters".to_owned()
        );
    };
    let answered = text.and_then(|held| {
        let offset = offset_of(held, params.text_document_position_params.position);
        let uri = params.text_document_position_params.text_document.uri;
        let path = std::path::PathBuf::from(uri.path().as_str());
        let sources = [rustmanifest_engine::Source::new(path, held.to_owned())];
        let facts = rustmanifest_engine::behavior(&sources);
        let graph = rustmanifest_engine::call_graph(
            &facts,
            &rustmanifest_engine::semantic::resolution_context(&sources)
        );
        let reached = rustmanifest_engine::transitive_effects(&facts, &graph);
        facts
            .iter()
            .enumerate()
            .find(|(_, fact)| {
                let range = fact.range;
                (range.start as usize..range.end as usize).contains(&offset)
            })
            .map(|(index, fact)| {
                let inherited = reached.get(index).map_or(&[][..], Vec::as_slice);
                let mut value = card(fact, inherited);
                let workspace = rustmanifest_engine::Workspace::new(&sources);
                for effect in inherited
                    .iter()
                    .filter(|effect| !fact.effects.contains(effect))
                {
                    let named = effect.to_string();
                    let asked = rustmanifest_engine::semantic::why_effect(
                        &workspace,
                        &fact.function,
                        &named
                    );
                    value.push_str("\n\n");
                    value.push_str(&why_section(&named, &asked));
                }
                Hover {
                    contents: HoverContents::Markup(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value
                    }),
                    range:    None
                }
            })
    });
    answered.map_or_else(
        || Response::new_ok(request.id.clone(), serde_json::Value::Null),
        |hover| Response::new_ok(request.id.clone(), hover)
    )
}
