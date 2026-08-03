// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Canonical JSON projection of behavioral facts.
//!
//! One function's facts become one JSON object with stable kebab-case
//! keys; the CLI `facts` subcommand and the MCP knowledge tools both
//! render through this projection, so the wire shape has a single source
//! of truth. The binarized feature vector is appended on request.

use crate::{
    semantic::{FunctionFacts, ScipEdges, call_graph_with, features, role, transitive_effects},
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

/// Renders one recorded write to an output stream, with everything the
/// analysis proved about it and nothing it did not.
///
/// `text` stays the fully proven string and is null whenever any part of
/// the payload is unproven; `payload` then says which part and why, so a
/// template keeps its proven shape and an unreadable payload keeps its
/// reason instead of both collapsing into the same null. `guards` lists
/// the constructs the site stands under, outermost first — the evidence
/// behind `certainty`.
fn output_json(write: &crate::semantic::OutputWrite) -> serde_json::Value {
    serde_json::json!({
        "stream": write.stream.to_string(),
        "text": write.payload.fully_known(),
        "payload": payload_json(&write.payload),
        "certainty": write.guard.local_grade().to_string(),
        "guards": write.guard.guards.iter().map(|guard| guard_name(*guard)).collect::<Vec<_>>(),
        "repeats": write.multiplicity == crate::semantic::Multiplicity::Unbounded,
        "newline": write.newline == crate::semantic::Newline::Appended,
        "site": { "start": write.site.start, "end": write.site.end }
    })
}

/// Renders what a write puts on the stream, keeping every part the
/// analysis proved and naming every part it did not.
///
/// A template keeps its proven text with each unproven position marked
/// `<name>`, and lists those names separately; an unreadable payload
/// carries the reason it could not be read. No unproven value is ever
/// stood in for.
fn payload_json(payload: &crate::semantic::Payload) -> serde_json::Value {
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
            let holes: Vec<String> = segments
                .iter()
                .filter_map(|segment| match segment {
                    Segment::Text(_) => None,
                    Segment::Hole(hole) => Some(hole_name(hole))
                })
                .collect();
            serde_json::json!({ "kind": "template", "shape": shape, "holes": holes })
        }
        Payload::Opaque(reason) => {
            serde_json::json!({ "kind": "opaque", "reason": reason_name(*reason) })
        }
    }
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

/// One construct standing between a write and unconditional execution,
/// as a stable key. A match arm exports its kind only: whether its
/// pattern is irrefutable is not proven at this tier.
const fn guard_name(guard: crate::semantic::Guard) -> &'static str {
    use crate::semantic::Guard;

    match guard {
        Guard::If => "if",
        Guard::Else => "else",
        Guard::MatchArm {
            ..
        } => "match-arm",
        Guard::For => "for",
        Guard::While => "while",
        Guard::Closure => "closure",
        Guard::NestedItem => "nested-item",
        Guard::CfgGated => "cfg-gated"
    }
}

/// Renders one function's facts as a JSON object, with the binarized
/// feature vector appended when requested.
#[must_use]
pub fn fact_json(
    entry: &FunctionFacts,
    sources: &[Source],
    with_features: bool
) -> serde_json::Value {
    let file = sources
        .get(entry.source)
        .map_or_else(String::new, |source| source.path().display().to_string());
    let mut object = serde_json::json!({
        "function": entry.function,
        "file": file,
        "reads": entry.reads,
        "writes": entry.writes,
        "builds": entry.builds,
        "constructs": entry.constructs,
        "error-types": entry.error_types,
        "effects": entry.effects.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "params": entry.params.iter().map(|param| serde_json::json!({
            "name": param.name,
            "type": param.type_name,
            "mode": param.mode.to_string(),
            "uses": param.uses,
            "field-reads": param.field_reads
        })).collect::<Vec<_>>(),
        "calls": entry.calls,
        "clones": entry.clones,
        "allocations": entry.allocations,
        "output-writes": entry.output_writes.iter().map(output_json).collect::<Vec<_>>(),
        "test": entry.flags.test(),
        "role": role(entry).to_string()
    });
    if with_features && let serde_json::Value::Object(fields) = &mut object {
        fields.insert(
            "features".to_owned(),
            features(entry)
                .iter()
                .map(|feature| {
                    serde_json::json!({
                        "name": feature.name,
                        "active": feature.active
                    })
                })
                .collect()
        );
    }
    object
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

    use super::fact_json;
    use crate::{semantic::behavior, source::Source};

    fn wallet_sources() -> [Source; 1] {
        [Source::new(
            PathBuf::from("src/lib.rs"),
            "struct Wallet { balance: i64 }\nimpl Wallet { fn credit(&mut self, amount: i64) -> \
             Result<(), String> { self.balance += amount; println!(\"credited\"); Ok(()) } }"
                .to_owned()
        )]
    }

    #[test]
    fn json_object_carries_every_fact_category() {
        let sources = wallet_sources();
        let facts = behavior(&sources);
        let object = fact_json(&facts[0], &sources, false);
        assert_eq!(object["function"], "Wallet::credit");
        assert_eq!(object["file"], "src/lib.rs");
        assert_eq!(object["writes"][0], "Wallet.balance");
        assert_eq!(object["error-types"][0], "String");
        assert_eq!(object["effects"][0], "console");
        assert_eq!(object["role"], "io");
        assert!(object["features"].is_null());
    }

    #[test]
    fn a_template_payload_keeps_its_shape_instead_of_collapsing_to_null() {
        let sources = [Source::new(
            PathBuf::from("src/lib.rs"),
            "fn report(amount: i64) { println!(\"credited {amount}\"); }".to_owned()
        )];
        let facts = behavior(&sources);
        let object = fact_json(&facts[0], &sources, false);
        let write = &object["output-writes"][0];
        assert!(write["text"].is_null(), "a template is not fully known");
        assert_eq!(write["payload"]["kind"], "template");
        assert_eq!(write["payload"]["shape"], "credited <amount>");
        assert_eq!(write["payload"]["holes"][0], "amount");
    }

    #[test]
    fn an_opaque_payload_states_why_it_could_not_be_read() {
        let sources = [Source::new(
            PathBuf::from("src/lib.rs"),
            "fn report() { println!(GREETING); }".to_owned()
        )];
        let facts = behavior(&sources);
        let object = fact_json(&facts[0], &sources, false);
        let write = &object["output-writes"][0];
        assert!(write["text"].is_null(), "an opaque payload is not known");
        assert_eq!(write["payload"]["kind"], "opaque");
        assert_eq!(write["payload"]["reason"], "non-literal-format");
    }

    #[test]
    fn a_guarded_write_exports_the_stack_it_stands_under() {
        let sources = [Source::new(
            PathBuf::from("src/lib.rs"),
            "fn report(flag: bool) { if flag { println!(\"hi\"); } }".to_owned()
        )];
        let facts = behavior(&sources);
        let object = fact_json(&facts[0], &sources, false);
        let write = &object["output-writes"][0];
        assert_eq!(write["certainty"], "possible");
        assert_eq!(write["guards"][0], "if");
    }

    #[test]
    fn an_unguarded_write_exports_an_empty_stack() {
        let sources = wallet_sources();
        let facts = behavior(&sources);
        let object = fact_json(&facts[0], &sources, false);
        let write = &object["output-writes"][0];
        assert_eq!(write["guards"].as_array().unwrap().len(), 0);
        assert_eq!(write["payload"]["kind"], "literal");
        assert_eq!(write["text"], "credited");
    }

    #[test]
    fn features_flag_appends_the_binarized_vector() {
        let sources = wallet_sources();
        let facts = behavior(&sources);
        let object = fact_json(&facts[0], &sources, true);
        let features = object["features"].as_array().unwrap();
        assert!(!features.is_empty());
        let returns_result = features
            .iter()
            .find(|entry| entry["name"] == "returns-result")
            .unwrap();
        assert_eq!(returns_result["active"], true);
    }
}
