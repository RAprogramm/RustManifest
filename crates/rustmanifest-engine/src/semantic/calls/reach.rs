// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The build-target constraint on candidates: which of them the caller's
//! compilation unit can actually link against.

use super::context::ResolutionContext;
use crate::semantic::behavior::FunctionFacts;

/// Keeps only the candidates the caller's build target can link
/// against, so an impossible edge never reaches the uniqueness decision.
///
/// A context without target information — one built by hand rather than
/// from sources — admits everything, which keeps the check an added
/// constraint rather than a hidden dependency.
pub(super) fn linkable(
    context: &ResolutionContext,
    facts: &[FunctionFacts],
    caller: usize,
    candidates: &[usize]
) -> Vec<usize> {
    if context.targets.is_empty() {
        return candidates.to_vec();
    }
    let Some(from) = facts
        .get(caller)
        .and_then(|fact| context.targets.get(fact.source))
    else {
        return candidates.to_vec();
    };
    candidates
        .iter()
        .copied()
        .filter(|&callee| {
            facts
                .get(callee)
                .and_then(|fact| context.targets.get(fact.source))
                .is_none_or(|to| from.reaches(to))
        })
        .collect()
}
