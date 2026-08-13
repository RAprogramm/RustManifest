// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The refusal rows as facts: which named row one refused candidate
//! lands in, decided in exactly one place.
//!
//! The accounting's doctrine says no refusal is derivable only by
//! subtraction; this module extends it one level down — no candidate's
//! ROW is derivable only by reconstruction. The decision function here
//! is the single point both the graph's counters and the per-candidate
//! report pass through, so the two cannot drift apart.

use std::fmt;


/// The named row one refused candidate lands in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefusalRow {
    /// The name is declared nowhere the workspace holds.
    Foreign,
    /// A `?::` name the workspace declares reachably: the receiver's
    /// type is what the reading could not settle.
    UnknownReceiver,
    /// The name is declared in the workspace but reaches the caller from
    /// nowhere: not linkable from its build target, or written private
    /// in another crate and therefore unnameable here.
    NamedElsewhere,
    /// Several linkable functions match: the gate refuses to guess.
    Ambiguous,
    /// The declared owner carries no such member.
    OwnerWithoutMember,
    /// The owner carries the member, granted by a derive or by a trait
    /// it implements, and no source in the slice holds its body — the
    /// text was written by the expansion the reading does not see.
    GrantedWithoutBody,
    /// No resolver settled which body the call reaches.
    ///
    /// The one refusal a call can carry now: which function a call reaches
    /// is the compiler's answer, and where it did not answer there is
    /// nothing to look up. Every other row named a way of guessing.
    Unsettled,
    /// The candidate names a variant of a workspace enum: a value is
    /// being built there, and nothing is being called. It is recorded
    /// rather than dropped, because a candidate that leaves the count
    /// without a word is the silent loss the accounting exists against.
    Construction
}

impl fmt::Display for RefusalRow {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Foreign => "foreign",
            Self::UnknownReceiver => "unknown-receiver",
            Self::NamedElsewhere => "named-elsewhere",
            Self::Ambiguous => "ambiguous",
            Self::OwnerWithoutMember => "owner-without-member",
            Self::GrantedWithoutBody => "granted-without-body",
            Self::Unsettled => "unsettled",
            Self::Construction => "construction"
        })
    }
}

