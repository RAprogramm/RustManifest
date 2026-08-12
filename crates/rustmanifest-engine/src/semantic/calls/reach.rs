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
/// Three constraints apply, all read from what the sources write. The
/// build target: a library function cannot call into a `tests` or
/// `examples` crate, and two integration test files are two crates. The
/// written visibility: an item declared without `pub` cannot be named
/// from another crate at all, so a same-named private method of a
/// neighbouring crate is not a candidate but a coincidence of spelling
/// — a trait-impl method is exempt, carrying no visibility of its own.
/// And compilation itself: a file no crate root reaches is a fixture or
/// a loose script that no target holds, so a compiled caller cannot
/// bind into it. That last one only prunes when the caller is itself
/// compiled, so analyzing a bare subtree — where every file is loose —
/// keeps every candidate.
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
    let Some(calling) = facts.get(caller) else {
        return candidates.to_vec();
    };
    let caller_source = calling.source;
    let Some(from) = context.targets.get(caller_source) else {
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
            from.reaches(to)
                && (nameable(fact) || from.same_crate(to))
                && compiled_together(context, caller_source, fact.source)
        })
        .collect()
}

/// Whether one build can hold both files: a caller a crate root reaches
/// cannot bind into a file none reaches.
fn compiled_together(context: &ResolutionContext, asking: usize, asked: usize) -> bool {
    let loose = |at: usize| context.loose.get(at).copied().unwrap_or(false);
    loose(asking) || !loose(asked)
}

/// Whether another crate may name the function at all: written `pub`, or
/// a trait-impl method, whose reach the trait governs rather than the
/// method itself.
const fn nameable(fact: &FunctionFacts) -> bool {
    fact.flags.public() || fact.flags.trait_impl()
}
