// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Canonical JSON projection of behavioral facts.
//!
//! One function's facts become one JSON object with stable kebab-case
//! keys; the CLI `facts` subcommand and the MCP knowledge tools both
//! render through this projection, so the wire shape has a single source
//! of truth. The binarized feature vector is appended on request.

use crate::{
    semantic::{FunctionFacts, call_graph, features, role, transitive_effects},
    source::Source
};

/// Renders every function's facts as one JSON array.
///
/// The canonical shape shared by the CLI and the MCP server, including
/// per-function transitive effects closed over the resolved call graph.
/// `filter` narrows by substring of the qualified name; `active` keeps
/// only functions with a write, effect, or error type.
#[must_use]
pub fn facts_json(
    sources: &[Source],
    filter: Option<&str>,
    active: bool,
    with_features: bool
) -> serde_json::Value {
    let facts = crate::semantic::behavior(sources);
    let graph = call_graph(&facts, &crate::semantic::resolution_segments(sources));
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
