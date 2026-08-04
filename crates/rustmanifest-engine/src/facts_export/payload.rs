// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Canonical JSON shape of what a write puts on a stream.
//!
//! The proven text of a payload and the names of its unproven positions
//! are the whole vocabulary here: a template keeps its shape with each
//! hole marked, an unreadable payload keeps the reason it stayed
//! unreadable. No unproven value is ever stood in for.

/// Renders what a write puts on the stream, keeping every part the
/// analysis proved and naming every part it did not.
///
/// A template keeps its proven text with each unproven position marked
/// `<name>`, and lists those names separately; an unreadable payload
/// carries the reason it could not be read. No unproven value is ever
/// stood in for.
pub(super) fn payload_json(payload: &crate::semantic::Payload) -> serde_json::Value {
    use crate::semantic::{Payload, Segment};

    match payload {
        Payload::Empty => serde_json::json!({ "kind": "empty" }),
        Payload::Literal(_) => serde_json::json!({ "kind": "literal" }),
        Payload::Template(segments) => {
            let shape: String = segments
                .iter()
                .map(|segment| match segment {
                    Segment::Text(text) => text.clone(),
                    Segment::Hole(hole) => format!("<{}>", hole_name(hole))
                })
                .collect();
            let holes: Vec<serde_json::Value> = segments
                .iter()
                .filter_map(|segment| match segment {
                    Segment::Text(_) => None,
                    Segment::Hole(hole) => Some(hole_json(hole))
                })
                .collect();
            serde_json::json!({ "kind": "template", "shape": shape, "holes": holes })
        }
        Payload::Opaque(reason) => {
            serde_json::json!({ "kind": "opaque", "reason": reason_name(*reason) })
        }
    }
}

/// Renders one unproven position: its name, the type of the value that
/// fills it where the tables resolved one, and what that type's display
/// implementation can put there where it was enumerated.
///
/// `alternatives` is absent rather than empty where nothing was
/// enumerated, so a consumer cannot read "not enumerated" as "renders
/// nothing".
fn hole_json(hole: &crate::semantic::Hole) -> serde_json::Value {
    let mut object = serde_json::json!({ "name": hole_name(hole) });
    let serde_json::Value::Object(fields) = &mut object else {
        return object;
    };
    if let Some(type_name) = &hole.type_name {
        fields.insert("type".to_owned(), serde_json::json!(type_name));
    }
    if !hole.alternatives.is_empty() {
        fields.insert(
            "alternatives".to_owned(),
            serde_json::json!(
                hole.alternatives
                    .iter()
                    .map(payload_json)
                    .collect::<Vec<_>>()
            )
        );
    }
    object
}

/// Names one unproven position by whatever the source gives it, never by
/// a value it does not have.
fn hole_name(hole: &crate::semantic::Hole) -> String {
    use crate::semantic::HoleSource;

    match &hole.source {
        HoleSource::Named(name) => name.clone(),
        HoleSource::Positional(index) | HoleSource::Indexed(index) => hole
            .expr
            .clone()
            .unwrap_or_else(|| format!("argument {index}"))
    }
}

/// Why a payload could not be recovered as text, as a stable key.
const fn reason_name(reason: crate::semantic::OpaqueReason) -> &'static str {
    use crate::semantic::OpaqueReason;

    match reason {
        OpaqueReason::RuntimeValue => "runtime-value",
        OpaqueReason::NonLiteralFormat => "non-literal-format",
        OpaqueReason::Unparsed => "unparsed",
        OpaqueReason::DebugMacro => "debug-macro"
    }
}
