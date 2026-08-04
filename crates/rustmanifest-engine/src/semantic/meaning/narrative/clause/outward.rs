// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The two clauses that rest on no origin: what the program writes, and
//! what it hands back.
//!
//! Neither can refuse. A write and a return position are recorded whole,
//! so nothing about them is left to a reading that could fail — which is
//! why these two answer with the clause or with nothing, and never with a
//! question.

use std::fmt::Write as _;

use super::{
    super::super::{OutputClaim, claim::HandbackClaim},
    opening
};
use crate::semantic::behavior::{Grade, Outcome};

/// What the program writes.
///
/// A site no chain from this entry reaches is counted rather than
/// narrated: it is a fact about the call graph, and stating it in the
/// same breath as what the program does would read as something the
/// program does.
pub(in crate::semantic::meaning::narrative) fn write_clause(
    claims: &[OutputClaim]
) -> Option<String> {
    if claims.is_empty() {
        return None;
    }
    let mut sentence = String::new();
    let mut unreached = 0_usize;
    for claim in claims {
        if claim.execution == Grade::Unreached {
            unreached += 1;
            continue;
        }
        let _ = write!(
            sentence,
            "{}{} {} to {}",
            if sentence.is_empty() { "" } else { ", " },
            opening(claim.execution, "writes"),
            claim.payload_text(),
            claim.write.stream
        );
    }
    if unreached > 0 {
        let _ = write!(
            sentence,
            "{}{unreached} further recorded write(s) no call chain from this entry reaches",
            if sentence.is_empty() { "" } else { "; " }
        );
    }
    Some(sentence)
}

/// What the program hands back to whoever called it.
///
/// Only the positions handing back a failure are counted: that a function
/// returns a value is its signature's statement, while that a failure
/// leaves it is this channel's, and only the second is news.
pub(in crate::semantic::meaning::narrative) fn handback_clause(
    claims: &[HandbackClaim]
) -> Option<String> {
    let failures = claims
        .iter()
        .filter(|claim| matches!(claim.handback.outcome, Outcome::Failure))
        .count();
    if failures == 0 {
        return None;
    }
    Some(format!(
        "{failures} recorded position(s) hand a failure back to the caller"
    ))
}
