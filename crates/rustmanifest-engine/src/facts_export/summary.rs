// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Canonical JSON shape of one function summary: where the value it hands
//! back comes from, written in terms of its own inputs.
//!
//! The summary is the artefact that lets provenance cross a call, so a
//! consumer that cannot read it cannot follow a value past the function
//! it was produced in. It ships as words rather than as a tree: the term
//! is a nested structure whose only stable reading is the sentence the
//! meaning tier already renders from it, and shipping a shape no consumer
//! could interpret would be shipping noise.

use crate::semantic::{FunctionSummary, origin_text};

/// Renders one summary as a JSON object.
///
/// `gap` is present only where the term failed on a question, so a
/// consumer cannot read a settled origin as a refused one.
pub(super) fn summary_json(summary: &FunctionSummary) -> serde_json::Value {
    let mut object = serde_json::json!({
        "slots": summary.slots,
        "result": origin_text(&summary.result),
        "derived": summary.result.derived()
    });
    if let (Some(gap), serde_json::Value::Object(fields)) = (summary.result.gap(), &mut object) {
        fields.insert("gap".to_owned(), serde_json::json!(gap.label()));
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

    use super::*;
    use crate::{
        semantic::{behavior, call_graph, resolution_context, summaries},
        source::Source
    };

    fn summary_of(text: &str, function: &str) -> FunctionSummary {
        let sources = [Source::new(PathBuf::from("src/lib.rs"), text.to_owned())];
        let facts = behavior(&sources);
        let graph = call_graph(&facts, &resolution_context(&sources));
        summaries(&sources, &facts, &graph)
            .into_iter()
            .find(|summary| summary.function == function)
            .expect("the function is summarized")
    }

    #[test]
    fn a_summary_states_its_slots_and_where_its_result_came_from() {
        let json = summary_json(&summary_of("pub fn go(value: u32) -> u32 { value }", "go"));
        assert_eq!(json["slots"], serde_json::json!([Some("value")]));
        assert_eq!(json["derived"], true);
        assert!(json["result"].as_str().unwrap().contains("value"));
        assert!(json.get("gap").is_none());
    }

    #[test]
    fn a_summary_that_failed_on_a_question_names_it() {
        let json = summary_json(&summary_of(
            "pub fn go(handle: &Thing) -> u32 { handle.unknown_call() }",
            "go"
        ));
        assert_eq!(json["derived"], false);
        assert!(json["gap"].is_string());
    }
}
