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

mod outward;

pub(super) use self::outward::{handback_clause, write_clause};
use super::super::{
    claim::{ChoiceClaim, ComputeClaim},
    intake::IntakeClaim
};
use crate::semantic::behavior::{Closure, Grade, Operation, OriginGap, Provenance, Selector};

/// What the program takes in, or the question the reading failed on.
pub(super) fn intake_clause(claims: &[IntakeClaim]) -> Result<Option<String>, OriginGap> {
    if claims.is_empty() {
        return Ok(None);
    }
    let mut named = Vec::new();
    for claim in claims {
        match &claim.read.selector {
            Selector::NotIdentified(_) => return Err(OriginGap::Unreadable),
            settled => named.push(format!("{} of its {}", settled, claim.read.channel))
        }
    }
    Ok(Some(format!(
        "{} {}",
        opening(weakest(claims.iter().map(|claim| claim.execution)), "takes"),
        named.join(", ")
    )))
}

/// What the program computes, or the question the reading failed on.
pub(super) fn compute_clause(claims: &[ComputeClaim]) -> Result<Option<String>, OriginGap> {
    if claims.is_empty() {
        return Ok(None);
    }
    let mut named = Vec::new();
    for claim in claims {
        if let Operation::Conversion(conversion) = &claim.operation.operation
            && conversion.target.is_none()
        {
            return Err(OriginGap::Unreadable);
        }
        if let Some(gap) = claim.operation.operands.iter().find_map(unsettled) {
            return Err(gap);
        }
        named.push(claim.operation.operation.to_string());
    }
    named.sort_unstable();
    named.dedup();
    Ok(Some(format!(
        "{} {}",
        opening(
            weakest(claims.iter().map(|claim| claim.execution)),
            "computes"
        ),
        named.join(", ")
    )))
}

/// What the program chooses between, or the question the reading failed
/// on.
pub(super) fn choice_clause(claims: &[ChoiceClaim]) -> Result<Option<String>, OriginGap> {
    if claims.is_empty() {
        return Ok(None);
    }
    let mut named = Vec::new();
    for claim in claims {
        if let Some(gap) = unsettled(&claim.choice.scrutinee) {
            return Err(gap);
        }
        let closed = if matches!(claim.choice.closure, Closure::CatchAll) {
            "of which one accepts every remaining value"
        } else {
            "not proven to be all of them"
        };
        named.push(format!(
            "{} written alternatives {closed}",
            claim.choice.alternatives.len()
        ));
    }
    Ok(Some(format!(
        "{} between {}",
        opening(
            weakest(claims.iter().map(|claim| claim.execution)),
            "chooses"
        ),
        named.join(", and between ")
    )))
}

/// The question an origin failed on, where it failed on one.
const fn unsettled(origin: &Provenance) -> Option<OriginGap> {
    match origin {
        Provenance::NotDerived(gap) => Some(*gap),
        _ => None
    }
}

/// The weakest grade among a set of claims.
///
/// A clause speaks of its sites together, and a group of sites is only as
/// certain as its least certain member: saying "unconditionally" of a set
/// containing one guarded site would state of all of them what holds of
/// some.
fn weakest(grades: impl Iterator<Item = Grade>) -> Grade {
    grades.min().unwrap_or(Grade::Unreached)
}

/// The opening words of one clause, fixed by how certainly it happens.
fn opening(grade: Grade, verb: &str) -> String {
    match grade {
        Grade::Certain => format!("the process unconditionally {verb}"),
        Grade::Possible => format!("the process may {}", verb.trim_end_matches('s')),
        Grade::Dead => format!(
            "a site that can never run would {}",
            verb.trim_end_matches('s')
        ),
        Grade::Unreached => format!(
            "a site no call chain from this entry reaches would {}",
            verb.trim_end_matches('s')
        )
    }
}
