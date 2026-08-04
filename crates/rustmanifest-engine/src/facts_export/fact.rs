// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Canonical JSON shape of one function's facts.
//!
//! Every fact category the analysis records for a single function becomes
//! one stable kebab-case key here: reads, writes, builds, constructs,
//! error types, effects, parameters, calls, clones, allocations, output
//! writes, the test flag and the behavioral role. The binarized feature
//! vector is appended on request.

use super::{output_write::output_json, process_exit::exit_json};
use crate::{
    semantic::{FunctionFacts, features, role},
    source::Source
};

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
        "exits": entry.exits.iter().map(exit_json).collect::<Vec<_>>(),
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
