// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The machine form: the same claims as data, with the byte range that
//! proves each of them and the recognizer set the build ran.

use rustmanifest_engine::{EntryMeaning, Multiplicity, OUTPUT_SINK_SET, OutputClaim};

use super::{
    location::relative,
    wording::{certainty, entry_kind}
};

/// Renders the report as data.
///
/// Every entry carries `sink-set`, the version of the recognizer list
/// this build ran, so an entry with no claim reads as a versioned
/// negative result rather than as an absence of writes.
pub(super) fn json_report(entries: &[EntryMeaning], root: &std::path::Path) -> serde_json::Value {
    let documents: Vec<serde_json::Value> = entries
        .iter()
        .map(|entry| {
            serde_json::json!({
                "entry": relative(&entry.file, root),
                "entry-kind": entry_kind(entry.kind),
                "sink-set": OUTPUT_SINK_SET,
                "claims": entry
                    .claims
                    .iter()
                    .map(|claim| claim_json(claim, root))
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    serde_json::Value::Array(documents)
}

/// Renders one claim as data, with every field the text form shows and
/// the byte range that proves it.
fn claim_json(claim: &OutputClaim, root: &std::path::Path) -> serde_json::Value {
    serde_json::json!({
        "function": claim.function,
        "file": relative(&claim.file, root),
        "stream": claim.write.stream.to_string(),
        "text": claim.payload_text(),
        "text-fully-known": claim.write.payload.fully_known().is_some(),
        "certainty": certainty(claim.execution),
        "reach": certainty(claim.reach),
        "repeats": claim.write.multiplicity == Multiplicity::Unbounded,
        "weakened-by": claim.weakening(),
        "site": { "start": claim.write.site.start, "end": claim.write.site.end },
        "sink-set": claim.sink_set
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
    use std::path::Path;

    use rustmanifest_engine::{EntryKind, EntryMeaning, OUTPUT_SINK_SET};

    use super::json_report;

    fn silent_entry() -> EntryMeaning {
        EntryMeaning {
            file:       "src/main.rs".to_owned(),
            kind:       EntryKind::Plain,
            claims:     Vec::new(),
            intake:     Vec::new(),
            completion: rustmanifest_engine::Completion::Nothing,
            exits:      Vec::new()
        }
    }

    #[test]
    fn an_entry_with_no_claim_carries_the_recognizer_set_as_data() {
        let report = json_report(&[silent_entry()], Path::new("."));
        assert_eq!(report[0]["claims"].as_array().unwrap().len(), 0);
        assert_eq!(report[0]["sink-set"], OUTPUT_SINK_SET);
    }
}
