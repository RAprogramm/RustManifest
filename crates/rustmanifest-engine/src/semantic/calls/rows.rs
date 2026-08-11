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

use std::{collections::BTreeSet, fmt};

use super::context::ResolutionContext;

/// The named row one refused candidate lands in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefusalRow {
    /// The name is declared nowhere the workspace holds.
    Foreign,
    /// A `?::` name the workspace declares reachably: the receiver's
    /// type is what the reading could not settle.
    UnknownReceiver,
    /// The name is declared in the workspace but links from nowhere the
    /// caller is built.
    NamedElsewhere,
    /// Several linkable functions match: the gate refuses to guess.
    Ambiguous,
    /// The declared owner carries no such member.
    OwnerWithoutMember
}

impl fmt::Display for RefusalRow {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Foreign => "foreign",
            Self::UnknownReceiver => "unknown-receiver",
            Self::NamedElsewhere => "named-elsewhere",
            Self::Ambiguous => "ambiguous",
            Self::OwnerWithoutMember => "owner-without-member"
        })
    }
}

/// Decides the row of one refused candidate — the single decision
/// point the counters and the per-candidate report both pass through.
pub(super) fn refusal_row(
    candidate: &str,
    named: bool,
    matches: Option<&[usize]>,
    workspace_owners: &BTreeSet<&str>,
    module_segments: &BTreeSet<String>,
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
        None => {
            let foreign = candidate.rsplit_once("::").map_or_else(
                || !named,
                |(owner, _)| {
                    !workspace_owners.contains(owner)
                        && !module_segments.contains(owner)
                        && !context.trait_impls.contains_key(owner)
                }
            );
            if foreign {
                RefusalRow::Foreign
            } else {
                RefusalRow::OwnerWithoutMember
            }
        }
    }
}
