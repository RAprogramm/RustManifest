// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! One clause of the compound statement each, built from the claims of
//! one channel.
//!
//! A builder answers with the clause, with nothing where the channel
//! recorded no site, or with the question it failed on. Only the clauses
//! resting on an origin can fail: a write and a return position are
//! recorded whole, and nothing about them is left to a reading that could
//! refuse.
//!
//! Each clause speaks per function, not over the whole program at once.
//! Certainty belongs to a site and reaches a group only through the
//! function holding it, so a clause covering two functions makes two
//! statements rather than one hedged to the weaker of them.

mod grouping;
mod outward;

use self::grouping::{by_function, joined};
pub(super) use self::outward::{handback_clause, write_clause};
use super::super::{
    claim::{ChoiceClaim, ComputeClaim},
    intake::IntakeClaim
};
use crate::semantic::behavior::{Closure, Grade, Operation, OriginGap, Provenance, Selector};

/// A site no path from the entry reaches says nothing about what the
/// process does, so a refusal standing there cannot break a statement
/// about the process. Its claim is kept — the report prints it and says
/// it is unreached — but the sentence is built from the sites the entry
/// actually reaches.
///
/// What the program takes in, or the question the reading failed on.
pub(super) fn intake_clause(claims: &[IntakeClaim]) -> Result<Option<String>, OriginGap> {
    let groups = by_function(
        claims,
        |claim| claim.function.as_str(),
        |claim| claim.execution
    );
    let mut parts = Vec::new();
    for held in groups {
        let mut named = Vec::new();
        for claim in held.sites {
            if claim.execution == Grade::Unreached {
                continue;
            }
            match &claim.read.selector {
                Selector::NotIdentified(_) => return Err(OriginGap::Unreadable),
                settled => named.push(format!("{} of its {}", settled, claim.read.channel))
            }
        }
        if named.is_empty() {
            continue;
        }
        parts.push(format!(
            "{} {} in `{}`",
            opening(held.grade, "takes"),
            joined(&named),
            held.function
        ));
    }
    Ok(sentence(&parts))
}

/// What the program computes, or the question the reading failed on.
///
/// The clause names the operations, not their operands, so an operand
/// the reading could not settle does not make what it says any less
/// true. What it does print is the target of a conversion, and a
/// conversion whose target is written nowhere refuses — the clause would
/// otherwise name a type the source never wrote. The operand refusals
/// are stated where they are actually said: in the write clause, beside
/// the position they fill.
pub(super) fn compute_clause(claims: &[ComputeClaim]) -> Result<Option<String>, OriginGap> {
    let groups = by_function(
        claims,
        |claim| claim.function.as_str(),
        |claim| claim.execution
    );
    let mut parts = Vec::new();
    for held in groups {
        let mut named = Vec::new();
        for claim in held.sites {
            if claim.execution == Grade::Unreached {
                continue;
            }
            if let Operation::Conversion(conversion) = &claim.operation.operation
                && conversion.target.is_none()
            {
                return Err(OriginGap::Unreadable);
            }
            named.push(claim.operation.operation.to_string());
        }
        named.sort_unstable();
        named.dedup();
        if named.is_empty() {
            continue;
        }
        parts.push(format!(
            "{} {} in `{}`",
            opening(held.grade, "computes"),
            joined(&named),
            held.function
        ));
    }
    Ok(sentence(&parts))
}

/// What the program chooses between, or the question the reading failed
/// on.
///
/// The clause names how many alternatives are written and on what ground
/// the set is closed, both of which the record proves whatever the
/// scrutinee's origin turned out to be.
pub(super) fn choice_clause(claims: &[ChoiceClaim]) -> Result<Option<String>, OriginGap> {
    let groups = by_function(
        claims,
        |claim| claim.function.as_str(),
        |claim| claim.execution
    );
    let mut parts = Vec::new();
    for held in groups {
        let mut named = Vec::new();
        for claim in held.sites {
            if claim.execution == Grade::Unreached {
                continue;
            }
            named.push(format!(
                "{} written alternatives {}",
                claim.choice.alternatives.len(),
                match claim.choice.closure {
                    Closure::CatchAll => "of which one accepts every remaining value",
                    Closure::StandardVariants => {
                        "which name every variant of a standard type"
                    }
                    Closure::Unproven => "not proven to be all of them"
                }
            ));
        }
        if named.is_empty() {
            continue;
        }
        parts.push(format!(
            "{} between {} in `{}`",
            opening(held.grade, "chooses"),
            joined(&named),
            held.function
        ));
    }
    Ok(sentence(&parts))
}

/// The clause a set of per-function parts amounts to, or nothing where
/// the channel recorded no site at all.
fn sentence(parts: &[String]) -> Option<String> {
    (!parts.is_empty()).then(|| parts.join("; "))
}

/// The question an origin failed on, where it failed on one.
const fn unsettled(origin: &Provenance) -> Option<OriginGap> {
    match origin {
        Provenance::NotDerived(gap) => Some(*gap),
        _ => None
    }
}

/// The opening words of one part, fixed by how certainly it happens.
fn opening(grade: Grade, verb: &str) -> String {
    match grade {
        Grade::Certain => format!("the process unconditionally {verb}"),
        Grade::Possible => format!("the process may {}", verb.trim_end_matches('s')),
        Grade::Dead => format!(
            "a site that can never run would {}",
            verb.trim_end_matches('s')
        ),
        Grade::Unreached => format!(
            "no call chain from this entry reaches a site that would {}",
            verb.trim_end_matches('s')
        )
    }
}
