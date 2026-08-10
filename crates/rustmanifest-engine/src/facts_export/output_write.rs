// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Canonical JSON shape of one recorded write to an output stream.
//!
//! The stream, the proven text, the payload behind it, the certainty and
//! the guard stack that earns that certainty, the machinery the payload
//! travelled through and where the stream's lock was taken, plus
//! multiplicity, newline and site. The payload itself is shaped by the
//! sibling module.

use super::payload::payload_json;

/// Renders one recorded write to an output stream, with everything the
/// analysis proved about it and nothing it did not.
///
/// `text` stays the fully proven string and is null whenever any part of
/// the payload is unproven; `payload` then says which part and why, so a
/// template keeps its proven shape and an unreadable payload keeps its
/// reason instead of both collapsing into the same null. `guards` lists
/// the constructs the site stands under, outermost first — the evidence
/// behind `certainty`. `conduit` names the machinery the payload reached
/// the stream through, and `lock` where the stream's lock was taken, so a
/// consumer can tell a formatted print from a direct byte write and a
/// per-write lock from a guard held across a run.
pub(super) fn output_json(write: &crate::semantic::OutputWrite) -> serde_json::Value {
    serde_json::json!({
        "stream": write.stream.to_string(),
        "text": write.payload.fully_known(),
        "payload": payload_json(&write.payload),
        "certainty": write.guard.local_grade().to_string(),
        "guards": write.guard.guards.iter().map(|guard| guard_name(*guard)).collect::<Vec<_>>(),
        "repeats": write.multiplicity == crate::semantic::Multiplicity::Unbounded,
        "newline": write.newline == crate::semantic::Newline::Appended,
        "conduit": write.conduit.label(),
        "lock": write.acquisition.label(),
        "buffering": match write.buffering {
            crate::semantic::Buffering::StreamOwn => "stream-own",
            crate::semantic::Buffering::Block => "block",
            crate::semantic::Buffering::Unknown => "unknown"
        },
        "site": { "start": write.site.start, "end": write.site.end }
    })
}

/// One construct standing between a write and unconditional execution,
/// as a stable key. A match arm exports its kind only: whether its
/// pattern is irrefutable is not proven at this tier.
const fn guard_name(guard: crate::semantic::Guard) -> &'static str {
    use crate::semantic::Guard;

    match guard {
        Guard::If => "if",
        Guard::Else => "else",
        Guard::MatchArm {
            ..
        } => "match-arm",
        Guard::For => "for",
        Guard::While => "while",
        Guard::Closure => "closure",
        Guard::NestedItem => "nested-item",
        Guard::CfgGated => "cfg-gated"
    }
}
