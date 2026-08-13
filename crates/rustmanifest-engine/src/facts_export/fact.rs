// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Canonical JSON shape of one function's facts.
//!
//! Every fact category the analysis records for a single function becomes
//! one stable kebab-case key here: reads, writes, builds, constructs,
//! error types, effects and the sites that proved them, parameters, calls
//! and the sites that make them, handbacks, clones, allocations, output
//! writes, the test flag and the behavioral role. The binarized feature
//! vector is appended on request.

use super::{
    call_site::{call_site_json, handback_json},
    choice::choice_json,
    effect_site::effect_site_json,
    input_read::read_json,
    operation::operation_json,
    output_write::output_json,
    process_exit::exit_json
};
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
        "built-fields": entry.built_fields.iter().map(|built| serde_json::json!({
            "owner": built.owner,
            "field": built.field,
            "origin": built.origin.label()
        })).collect::<Vec<_>>(),
        "error-types": entry.error_types,
        "effects": entry.effects.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "effect-sites": entry.effect_sites.iter().map(effect_site_json).collect::<Vec<_>>(),
        "params": entry.params.iter().map(|param| serde_json::json!({
            "name": param.name,
            "type": param.type_name,
            "mode": param.mode.to_string(),
            "uses": param.uses,
            "field-reads": param.field_reads
        })).collect::<Vec<_>>(),
        "calls": entry.calls,
        "call-sites": entry.call_sites.iter().map(call_site_json).collect::<Vec<_>>(),
        "handbacks": entry.handbacks.iter().map(handback_json).collect::<Vec<_>>(),
        "clones": entry.clones,
        "allocations": entry.allocations,
        "input-reads": entry.input_reads.iter().map(read_json).collect::<Vec<_>>(),
        "operations": entry.operations.iter().map(operation_json).collect::<Vec<_>>(),
        "choices": entry.choices.iter().map(choice_json).collect::<Vec<_>>(),
        "output-writes": entry.output_writes.iter().map(output_json).collect::<Vec<_>>(),
        "exits": entry.exits.iter().map(exit_json).collect::<Vec<_>>(),
        "queries": entry.queries.iter().map(|site| serde_json::json!({
            "statement": site.statement.to_string(),
            "table": site.table,
            "record": site.record,
            "binds": site.binds
        })).collect::<Vec<_>>(),
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
    use crate::{
        semantic::behavior_with,
        source::Source
    };

    /// What a compiler settles about a field written as `fragment`: the
    /// type holding it and the type it carries.
    ///
    /// The text says `self.balance` and nothing more; which type that
    /// `self` is, is the compiler's answer, and every fact naming the
    /// field by its owner rests on it.
    fn settled_field(
        text: &str,
        fragment: &str,
        owner: &str,
        produced: &str
    ) -> rustmanifest_schema::Resolution {
        let mut held = rustmanifest_schema::Resolution {
            resolver: "test".to_owned(),
            ..rustmanifest_schema::Resolution::default()
        };
        let mut cursor = 0usize;
        while let Some(found) = text[cursor..].find(fragment) {
            let start = cursor + found;
            cursor = start + fragment.len();
            held.fields.push(rustmanifest_schema::ResolvedField {
                at:       rustmanifest_schema::Location {
                    file:  PathBuf::from("src/lib.rs"),
                    range: rustmanifest_schema::Range {
                        start: u32::try_from(start).expect("a fixture is small"),
                        end:   u32::try_from(cursor).expect("a fixture is small")
                    }
                },
                owner:    owner.to_owned(),
                produced: produced.to_owned()
            });
        }
        crate::semantic::settling::settle_declared(&mut held, &wallet_sources());
        crate::semantic::settling::settle_standard_macros(&mut held, &wallet_sources());
        held.settle();
        held
    }

    const WALLET: &str = "struct Wallet { balance: i64 }\nimpl Wallet { fn credit(&mut self, \
                          amount: i64) -> Result<(), String> { self.balance += amount; \
                          println!(\"credited\"); Ok(()) } }";

    fn wallet_sources() -> [Source; 1] {
        [Source::new(PathBuf::from("src/lib.rs"), WALLET.to_owned())]
    }

    /// A field is named by the type holding it, and which type that is,
    /// is the compiler's answer: the text writes `self`.
    #[test]
    fn json_object_carries_every_fact_category() {
        let sources = wallet_sources();
        let resolution = settled_field(WALLET, "self.balance", "Wallet", "i64");
        let facts = behavior_with(&sources, Some(&resolution));
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
        let facts = crate::semantic::behavior_with(&sources, Some(&crate::semantic::settling::standard_macros_over(&sources)));
        let object = fact_json(&facts[0], &sources, false);
        let write = &object["output-writes"][0];
        assert!(write["text"].is_null(), "a template is not fully known");
        assert_eq!(write["payload"]["kind"], "template");
        assert_eq!(write["payload"]["shape"], "credited <amount>");
        assert_eq!(write["payload"]["holes"][0]["name"], "amount");
        assert!(
            write["payload"]["holes"][0]["alternatives"].is_null(),
            "nothing enumerated the position, and an absent list says so"
        );
    }

    #[test]
    fn an_opaque_payload_states_why_it_could_not_be_read() {
        let sources = [Source::new(
            PathBuf::from("src/lib.rs"),
            "fn report() { println!(GREETING); }".to_owned()
        )];
        let facts = crate::semantic::behavior_with(&sources, Some(&crate::semantic::settling::standard_macros_over(&sources)));
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
        let facts = crate::semantic::behavior_with(&sources, Some(&crate::semantic::settling::standard_macros_over(&sources)));
        let object = fact_json(&facts[0], &sources, false);
        let write = &object["output-writes"][0];
        assert_eq!(write["certainty"], "possible");
        assert_eq!(write["guards"][0], "if");
    }

    #[test]
    fn an_unguarded_write_exports_an_empty_stack() {
        let sources = wallet_sources();
        let facts = crate::semantic::behavior_with(&sources, Some(&crate::semantic::settling::standard_macros_over(&sources)));
        let object = fact_json(&facts[0], &sources, false);
        let write = &object["output-writes"][0];
        assert_eq!(write["guards"].as_array().unwrap().len(), 0);
        assert_eq!(write["payload"]["kind"], "literal");
        assert_eq!(write["text"], "credited");
    }

    #[test]
    fn features_flag_appends_the_binarized_vector() {
        let sources = wallet_sources();
        let facts = crate::semantic::behavior_with(&sources, Some(&crate::semantic::settling::standard_macros_over(&sources)));
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
