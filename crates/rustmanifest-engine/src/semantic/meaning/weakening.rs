// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What stands between a claim and certainty, named in words.
//!
//! Two weakenings are independent and a claim can carry both: the call
//! chain that leads to the owning function, and the constructs standing
//! over the site inside that function. Each is named on its own so a
//! doubly weakened claim never reads as a singly weakened one, and each
//! construct is named once however many times it stands there.

use std::collections::BTreeSet;

use super::OutputClaim;
use crate::semantic::behavior::{Grade, Guard, SiteGuard};

/// Names everything that stands between a claim and certainty.
///
/// Reach and local execution are two independent weakenings and a claim
/// can carry both; naming only the first found would hide the other and
/// report a doubly weakened claim as singly weakened. The order is
/// outside-in: the call chain first, then the constructs standing over
/// the site inside its own function.
pub(super) fn weakening_of(claim: &OutputClaim) -> Option<String> {
    if claim.execution == Grade::Certain {
        return None;
    }
    let reasons: Vec<String> = chain_reason(claim.reach)
        .into_iter()
        .map(ToOwned::to_owned)
        .chain(site_reasons(&claim.write.guard))
        .collect();
    (!reasons.is_empty()).then(|| reasons.join(" and "))
}

/// What the call chain does to a claim, or `None` when it does nothing.
pub(super) const fn chain_reason(reach: Grade) -> Option<&'static str> {
    match reach {
        Grade::Certain => None,
        Grade::Possible => Some("a call chain that does not certainly reach it"),
        Grade::Unreached => Some("a call chain that never reaches it"),
        Grade::Dead => Some("a call chain whose every call to it can never run")
    }
}

/// Every construct standing over the site inside its own function, each
/// named once however many times it stands there.
pub(super) fn site_reasons(guard: &SiteGuard) -> Vec<String> {
    let named: BTreeSet<&'static str> = guard
        .guards
        .iter()
        .map(|guard| match guard {
            Guard::If | Guard::Else => "a branch",
            Guard::MatchArm {
                ..
            } => "a match arm",
            Guard::For | Guard::While => "a loop that may not run",
            Guard::Closure => "a closure that runs only where it is called",
            Guard::NestedItem => "a nested item that runs only when called",
            Guard::CfgGated => "a configuration gate this analysis does not resolve"
        })
        .collect();
    let mut reasons: Vec<String> = named.into_iter().map(ToOwned::to_owned).collect();
    if guard.dead {
        reasons.push("a divergence before it that always leaves the function".to_owned());
    }
    if guard.prior_exits > 0 {
        reasons.push("an exit that may leave the function first".to_owned());
    }
    reasons
}
