// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Canonical JSON projection of behavioral facts.
//!
//! One function's facts become one JSON object with stable kebab-case
//! keys; the CLI `facts` subcommand and the MCP knowledge tools both
//! render through this projection, so the wire shape has a single source
//! of truth. The binarized feature vector is appended on request.
//!
//! The per-function object, the recorded output writes inside it and the
//! payload of each write each own a submodule; this module selects which
//! functions reach the wire and closes their effects over the call graph.

mod fact;
mod input_read;
mod output_write;
mod payload;
mod process_exit;

pub use self::fact::fact_json;
use crate::{
    semantic::{ScipEdges, call_graph_with, transitive_effects},
    source::Source
};

/// Renders every function's facts as one JSON array.
///
/// The canonical shape shared by the CLI and the MCP server, including
/// per-function transitive effects closed over the resolved call graph.
/// `filter` narrows by substring of the qualified name; `active` keeps
/// only functions with a write, effect, or error type. A SCIP index in
/// `scip` overlays name-resolved call edges onto the graph before the
/// effect closure, so transitive effects flow over edges the syntactic
/// resolver missed.
#[must_use]
pub fn facts_json(
    sources: &[Source],
    filter: Option<&str>,
    active: bool,
    with_features: bool,
    scip: Option<&ScipEdges>
) -> serde_json::Value {
    let facts = crate::semantic::behavior(sources);
    let overlay = scip.map(|edges| edges.candidates(&facts, sources));
    let graph = call_graph_with(
        &facts,
        &crate::semantic::resolution_context(sources),
        overlay.as_deref()
    );
    let transitive = transitive_effects(&facts, &graph);
    let selected: Vec<serde_json::Value> = facts
        .iter()
        .enumerate()
        .filter(|(_, entry)| filter.is_none_or(|needle| entry.function.contains(needle)))
        .filter(|(_, entry)| {
            !active
                || !entry.writes.is_empty()
                || !entry.effects.is_empty()
                || !entry.error_types.is_empty()
        })
        .map(|(index, entry)| {
            let mut object = fact_json(entry, sources, with_features);
            if let serde_json::Value::Object(fields) = &mut object {
                fields.insert(
                    "transitive-effects".to_owned(),
                    transitive[index]
                        .iter()
                        .map(|effect| serde_json::Value::String(effect.to_string()))
                        .collect()
                );
            }
            object
        })
        .collect();
    serde_json::Value::Array(selected)
}
