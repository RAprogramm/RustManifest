// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Canonical JSON shape of one recorded choice among written
//! alternatives.
//!
//! Where the value chosen on came from, one entry per alternative in
//! written order, and whether the alternatives are the whole of what can
//! happen — stated outright, because a set recorded as it stands and a
//! set proven closed license different statements.

/// Renders one recorded choice with everything the analysis proved about
/// it and nothing it did not.
pub(super) fn choice_json(choice: &crate::semantic::Choice) -> serde_json::Value {
    serde_json::json!({
        "chosen-on": choice.scrutinee.label(),
        "closed": matches!(choice.closure, crate::semantic::Closure::CatchAll),
        "alternatives": choice.alternatives.iter().map(|alternative| serde_json::json!({
            "pattern": alternative.pattern,
            "accepts-everything": alternative.irrefutable,
            "conditional": alternative.conditional,
            "value": alternative.value.label()
        })).collect::<Vec<_>>(),
        "certainty": choice.guard.local_grade().to_string(),
        "guards": choice.guard.guards.len(),
        "repeats": choice.multiplicity == crate::semantic::Multiplicity::Unbounded,
        "site": { "start": choice.site.start, "end": choice.site.end }
    })
}
