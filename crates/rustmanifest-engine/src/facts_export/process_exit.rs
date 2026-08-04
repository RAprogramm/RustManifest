// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Canonical JSON shape of one recorded site producing a process status.
//!
//! The status, how it leaves, the guard stack standing over the site, and
//! the site itself — the same shape a recorded write carries, because the
//! two are observed on the same terms.

/// Renders one recorded exit site with everything the analysis proved
/// about it and nothing it did not.
///
/// `status` is `computed` where the number is not written in the source:
/// that a status is produced here is proven, which one it is is not.
pub(super) fn exit_json(exit: &crate::semantic::ProcessExit) -> serde_json::Value {
    serde_json::json!({
        "status": exit.status.to_string(),
        "route": exit.route.to_string(),
        "certainty": exit.guard.local_grade().to_string(),
        "guards": exit.guard.guards.len(),
        "site": { "start": exit.site.start, "end": exit.site.end }
    })
}
