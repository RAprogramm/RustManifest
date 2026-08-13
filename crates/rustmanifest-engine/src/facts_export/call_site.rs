// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Canonical JSON shape of one recorded call site and one recorded
//! handback.
//!
//! The candidate list already exported says *which* functions a body
//! calls. It cannot say how often, under what, with what, or what the
//! source does about a failure — and those are the questions a caller's
//! own certainty rests on. These two channels carry them, so the export
//! ships what the fact base actually holds rather than a summary of it.

use crate::semantic::{Arguments, CallSite, Handback, Provenance};

/// Renders one call site: who is called, what it was handed, what stands
/// over it, and what the source does with a failure it can hand back.
///
/// `arguments` is absent rather than empty where the site is one no
/// source wrote — the error conversion behind a `?`, the poll an `await`
/// stands for — because an empty list would read as a call of no
/// arguments, which is a different fact.
pub(super) fn call_site_json(site: &CallSite) -> serde_json::Value {
    let mut object = serde_json::json!({
        "candidate": site.candidate,
        "disposal": site.disposal.label(),
        "certainty": site.guard.local_grade().to_string(),
        "guards": site.guard.guards.len(),
        "conditions": site.conditions.len(),
        "site": { "start": site.site.start, "end": site.site.end }
    });
    if let (Arguments::Written(origins), serde_json::Value::Object(fields)) =
        (&site.arguments, &mut object)
    {
        fields.insert(
            "arguments".to_owned(),
            serde_json::json!(origins.iter().map(Provenance::label).collect::<Vec<_>>())
        );
    }
    object
}

/// Renders one handback: where the value came from, which half of a
/// fallible return it is, and what stands over the position.
pub(super) fn handback_json(handback: &Handback) -> serde_json::Value {
    serde_json::json!({
        "value": handback.value.label(),
        "outcome": handback.outcome.to_string(),
        "certainty": handback.guard.local_grade().to_string(),
        "guards": handback.guard.guards.len(),
        "conditions": handback.conditions.len(),
        "site": { "start": handback.site.start, "end": handback.site.end }
    })
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
        semantic::{FunctionFacts, behavior},
        source::Source
    };

    fn facts(text: &str) -> Vec<FunctionFacts> {
        {
            let sources = [Source::new(PathBuf::from("src/lib.rs"), text.to_owned())];
            let settled = crate::semantic::settling::standard_macros_over(&sources);
            crate::semantic::behavior_with(&sources, Some(&settled))
        }
    }

    fn named(all: &[FunctionFacts], name: &str) -> FunctionFacts {
        all.iter()
            .find(|fact| fact.function == name)
            .cloned()
            .expect("the function is recorded")
    }

    #[test]
    fn a_written_call_states_what_it_was_handed() {
        let all = facts("fn inner(value: u32) -> u32 { value }\nfn outer() -> u32 { inner(7) }");
        let site = named(&all, "outer")
            .call_sites
            .into_iter()
            .find(|site| site.candidate == "inner")
            .expect("the call is recorded");
        let json = call_site_json(&site);
        assert_eq!(json["candidate"], "inner");
        assert_eq!(json["arguments"], serde_json::json!(["written"]));
        assert!(json["site"]["end"].as_u64() > json["site"]["start"].as_u64());
    }

    /// A site no source wrote has no argument list to read, and says so by
    /// leaving the key out rather than by an empty one.
    #[test]
    fn a_site_no_source_wrote_states_no_arguments() {
        let all = facts(
            "fn inner() -> Result<u32, u8> { Ok(1) }\nfn outer() -> Result<u32, u16> { \
             Ok(inner()?) }"
        );
        let unwritten = named(&all, "outer")
            .call_sites
            .into_iter()
            .find(|site| matches!(site.arguments, Arguments::Unwritten));
        if let Some(site) = unwritten {
            assert!(call_site_json(&site).get("arguments").is_none());
        }
    }

    #[test]
    fn a_handback_states_where_its_value_came_from() {
        let all = facts("fn go(value: u32) -> u32 { value }");
        let handback = named(&all, "go")
            .handbacks
            .into_iter()
            .next()
            .expect("the tail hands a value back");
        let json = handback_json(&handback);
        assert_eq!(json["value"], "parameter");
        assert!(json["certainty"].is_string());
    }
}
