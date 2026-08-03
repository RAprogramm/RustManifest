// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Closure of the direct per-function effect sets over the call graph.

use super::graph::CallGraph;
use crate::semantic::behavior::{Effect, FunctionFacts};

/// Closes per-function effect sets over the call graph to a fixpoint.
///
/// The result at position `i` is the direct effects of `facts[i]` united
/// with the effects of everything it transitively calls, sorted and
/// deduplicated.
#[must_use]
pub fn transitive_effects(facts: &[FunctionFacts], graph: &CallGraph) -> Vec<Vec<Effect>> {
    let mut sets: Vec<Vec<Effect>> = facts.iter().map(|fact| fact.effects.clone()).collect();
    let mut changed = true;
    while changed {
        changed = false;
        for caller in 0..sets.len() {
            for &callee in graph.callees(caller) {
                let additions: Vec<Effect> = sets[callee]
                    .iter()
                    .filter(|effect| !sets[caller].contains(effect))
                    .copied()
                    .collect();
                if !additions.is_empty() {
                    sets[caller].extend(additions);
                    changed = true;
                }
            }
        }
    }
    for set in &mut sets {
        set.sort();
        set.dedup();
    }
    sets
}
