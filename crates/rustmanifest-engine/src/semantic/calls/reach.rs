// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The reach constraints on candidates: which of them the caller's
//! compilation unit can link against, and which of them its crate is
//! even allowed to name.

use super::context::ResolutionContext;
use crate::semantic::behavior::FunctionFacts;

/// Keeps only the candidates the caller can reach, so an impossible edge
/// never arrives at the uniqueness decision.
///
/// Two constraints apply, both read from what the sources write. The
/// build target: a library function cannot call into a `tests` or
/// `examples` crate, and two integration test files are two crates. And
/// the written visibility: an item declared without `pub` cannot be
/// named from another crate at all, so a same-named private method of a
/// neighbouring crate is not a candidate but a coincidence of spelling.
/// A trait-impl method is exempt from the second — it carries no
/// visibility of its own, the trait's reach is what governs it.
///
/// A context without target information — one built by hand rather than
/// from sources — admits everything, which keeps the checks an added
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
            let Some(fact) = facts.get(callee) else {
                return true;
            };
            let Some(to) = context.targets.get(fact.source) else {
                return true;
            };
            from.reaches(to) && (nameable(fact) || from.same_crate(to))
        })
        .collect()
}

/// Whether another crate may name the function at all: written `pub`, or
/// a trait-impl method, whose reach the trait governs rather than the
/// method itself.
const fn nameable(fact: &FunctionFacts) -> bool {
    fact.flags.public() || fact.flags.trait_impl()
}
