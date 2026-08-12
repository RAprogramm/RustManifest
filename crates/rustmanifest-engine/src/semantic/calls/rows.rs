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

use super::context::ResolutionContext;

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
            Self::Construction => "construction"
        })
    }
}

/// Decides the row of one refused candidate — the single decision
/// point the counters and the per-candidate report both pass through.
pub(super) fn refusal_row(
    candidate: &str,
    named: bool,
    matches: Option<&[usize]>,
    owned_here: &dyn Fn(&str) -> bool,
    context: &ResolutionContext
) -> RefusalRow {
    if candidate.strip_prefix("?::").is_some() {
        return match matches {
            Some([]) => RefusalRow::NamedElsewhere,
            Some(_) => RefusalRow::UnknownReceiver,
            None => RefusalRow::Foreign
        };
    }
    match matches {
        Some([]) => RefusalRow::NamedElsewhere,
        Some(_) => RefusalRow::Ambiguous,
        None => unowned(candidate, named, owned_here, context)
    }
}

/// The row of a candidate no workspace function carries the name of.
///
/// A variant of a workspace enum is a value being built, and gets the
/// row that says so. Otherwise the question is whether an owner that
/// could carry a member is written where the caller can reach it: a
/// type the workspace gives methods to, or a trait it declares. A
/// module qualifier is neither — a module carries no members, and where
/// the workspace declares no function of that bare name, the written
/// path names an outside namespace of the same spelling. Reading a
/// module as an owner filed a third of this row under a lack the
/// workspace never had; reading an unreachable declaration as one filed
/// a fixture's type as the caller's.
fn unowned(
    candidate: &str,
    named: bool,
    owned_here: &dyn Fn(&str) -> bool,
    context: &ResolutionContext
) -> RefusalRow {
    let Some((owner, member)) = candidate.rsplit_once("::") else {
        return if named {
            RefusalRow::OwnerWithoutMember
        } else {
            RefusalRow::Foreign
        };
    };
    if context
        .variants
        .get(owner)
        .is_some_and(|written| written.contains(member))
    {
        return RefusalRow::Construction;
    }
    if granting(context, owner, member) {
        return RefusalRow::GrantedWithoutBody;
    }
    if owned_here(owner) || context.trait_impls.contains_key(owner) {
        RefusalRow::OwnerWithoutMember
    } else {
        RefusalRow::Foreign
    }
}

/// Whether a trait the owner carries without writing a body grants the
/// named member: a derive above the type, or an implemented trait whose
/// blanket companion provides the member — `Display` granting
/// `to_string` is the written case of that.
fn granting(context: &ResolutionContext, owner: &str, member: &str) -> bool {
    const GRANTED_MEMBERS: [(&str, &str); 8] = [
        ("clone", "Clone"),
        ("default", "Default"),
        ("cmp", "Ord"),
        ("partial_cmp", "PartialOrd"),
        ("eq", "PartialEq"),
        ("ne", "PartialEq"),
        ("hash", "Hash"),
        ("to_string", "Display")
    ];
    context.granted.get(owner).is_some_and(|carried| {
        GRANTED_MEMBERS
            .iter()
            .any(|(named, granting)| *named == member && carried.contains(*granting))
    })
}
