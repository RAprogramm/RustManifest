// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The effect-ban constraint: where a side-effect category is forbidden.
//!
//! An invariant carrying `effect` plus `deny-in` states that no function
//! in the listed modules may reach the effect — directly in its own body,
//! or through anything it calls. A transitive violation carries the
//! resolved call chain down to the function performing the effect, so the
//! ban never fires without showing the path it fired on.

use rustmanifest_schema::{Invariant, TraceStep};

use super::{
    chain::{edge_reason, effect_chain},
    scope::covered_by_any
};
use crate::semantic::{
    behavior::{Effect, FunctionFacts},
    calls::CallGraph
};

/// Checks the effect-ban constraint of one invariant against one
/// function's direct and transitive effects.
pub(super) fn effect_violation(
    invariant: &Invariant,
    facts: &[FunctionFacts],
    fact_index: usize,
    reachable: &[Effect],
    graph: &CallGraph,
    module: &str
) -> Option<(String, Vec<TraceStep>)> {
    let effect = invariant.effect.as_deref()?;
    let fact = facts.get(fact_index)?;
    if !reachable.iter().any(|observed| observed.covered_by(effect)) {
        return None;
    }
    if !covered_by_any(module, &invariant.deny_in) {
        return None;
    }
    let direct = fact
        .effects
        .iter()
        .any(|observed| observed.covered_by(effect));
    let denied = invariant.deny_in.join(", ");
    let mut trace = Vec::new();
    let how = if direct {
        trace.push(TraceStep {
            fact:   format!("fn `{}` performs `{effect}`", fact.function),
            reason: "observed effect-sink call or block in the body".to_owned()
        });
        String::new()
    } else {
        let chain = effect_chain(facts, graph, fact_index, effect);
        for window in chain.windows(2) {
            let reason = edge_reason(graph, window[0], window[1]);
            trace.push(TraceStep {
                fact:   format!(
                    "fn `{}` calls fn `{}`",
                    facts[window[0]].function, facts[window[1]].function
                ),
                reason: reason.to_owned()
            });
        }
        if let Some(&sink) = chain.last() {
            trace.push(TraceStep {
                fact:   format!("fn `{}` performs `{effect}`", facts[sink].function),
                reason: "observed effect-sink call or block in the body".to_owned()
            });
            format!(" through `{}`", facts[sink].function)
        } else {
            String::new()
        }
    };
    let verb = if direct {
        "performs"
    } else {
        "transitively performs"
    };
    let detail = format!(
        "invariant violated: `{}` {verb} `{effect}`{how} inside module `{module}`, but the \
         effect is denied in [{denied}]",
        fact.function
    );
    trace.push(TraceStep {
        fact:   format!("fn `{}` lives in module `{module}`", fact.function),
        reason: "module graph maps its file to this path".to_owned()
    });
    trace.push(TraceStep {
        fact:   format!("denied in: [{denied}]"),
        reason: "module is covered by a denied prefix".to_owned()
    });
    Some((detail, trace))
}
