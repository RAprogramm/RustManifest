// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Canonical JSON shape of one recorded operation over values.
//!
//! What the operation does, how many values it does it to, what the
//! construct does about the failure it can have, the type it produces
//! where the source names one, and the certainty and guard stack behind
//! it — the same shape every per-site record carries.

/// Renders one recorded operation with everything the analysis proved
/// about it and nothing it did not.
///
/// `produces` is absent rather than null where the source names no type,
/// so a consumer cannot read "not written" as "produces nothing".
pub(super) fn operation_json(site: &crate::semantic::OperationSite) -> serde_json::Value {
    let mut object = serde_json::json!({
        "operation": site.operation.to_string(),
        "operands": site.operands.iter().map(crate::semantic::Provenance::label).collect::<Vec<_>>(),
        "guarding": site.guarding.to_string(),
        "disposal": site.disposal.label(),
        "certainty": site.guard.local_grade().to_string(),
        "guards": site.guard.guards.len(),
        "repeats": site.multiplicity == crate::semantic::Multiplicity::Unbounded,
        "site": { "start": site.site.start, "end": site.site.end }
    });
    if let (Some(produced), serde_json::Value::Object(fields)) = (&site.produced, &mut object) {
        fields.insert("produces".to_owned(), serde_json::json!(produced));
    }
    object
}
