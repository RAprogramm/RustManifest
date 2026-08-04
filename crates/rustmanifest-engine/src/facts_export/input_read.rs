// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Canonical JSON shape of one recorded read from outside the process.
//!
//! The channel, the part drawn, the shape taken, the machinery it arrived
//! by, what became of the line terminator, the certainty and the guard
//! stack that earns it, plus multiplicity and site — the mirror of a
//! recorded write, because the two are observed on the same terms.

/// Renders one recorded read with everything the analysis proved about it
/// and nothing it did not.
///
/// `part` states which part of the channel is drawn, and says outright
/// that it is not identified rather than standing in a position the
/// source never wrote.
pub(super) fn read_json(read: &crate::semantic::InputRead) -> serde_json::Value {
    serde_json::json!({
        "channel": read.channel.to_string(),
        "part": read.selector.to_string(),
        "shape": read.shape.to_string(),
        "intake": read.intake.to_string(),
        "terminator": read.terminator.to_string(),
        "certainty": read.guard.local_grade().to_string(),
        "guards": read.guard.guards.len(),
        "repeats": read.multiplicity == crate::semantic::Multiplicity::Unbounded,
        "lock": read.acquisition.label(),
        "site": { "start": read.site.start, "end": read.site.end }
    })
}
