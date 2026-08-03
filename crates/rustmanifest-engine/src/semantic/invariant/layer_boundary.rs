// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The layer constraint: which module may reach which.
//!
//! An invariant carrying `call-deny-from` plus `call-deny-to` draws a
//! boundary between two module subtrees. A function below the constrained
//! prefix violates it the moment the call graph reaches any function below
//! the denied prefix, and the derivation walks that reach chain hop by hop
//! before naming both endpoints' modules.

use rustmanifest_schema::{Invariant, TraceStep};

use super::{
    chain::{edge_reason, reach_chain},
    scope::covered_by_any
};
use crate::semantic::{behavior::FunctionFacts, calls::CallGraph};

/// Checks the layer constraint of one invariant: functions under
/// `call-deny-from` must not reach functions under `call-deny-to`.
pub(super) fn call_violation(
    invariant: &Invariant,
    facts: &[FunctionFacts],
    fact_index: usize,
    graph: &CallGraph,
    module: &str,
    module_of: &dyn Fn(usize) -> String
) -> Option<(String, Vec<TraceStep>)> {
    let from = invariant.call_deny_from.as_deref()?;
    let to = invariant.call_deny_to.as_deref()?;
    if !covered_by_any(module, std::slice::from_ref(&from.to_owned())) {
        return None;
    }
    let fact = facts.get(fact_index)?;
    let chain = reach_chain(facts, graph, fact_index, |target| {
        covered_by_any(
            &module_of(facts[target].source),
            std::slice::from_ref(&to.to_owned())
        )
    })?;
    let sink = *chain.last()?;
    let mut trace = Vec::new();
    for window in chain.windows(2) {
        trace.push(TraceStep {
            fact:   format!(
                "fn `{}` calls fn `{}`",
                facts[window[0]].function, facts[window[1]].function
            ),
            reason: edge_reason(graph, window[0], window[1]).to_owned()
        });
    }
    trace.push(TraceStep {
        fact:   format!("fn `{}` lives under `{to}`", facts[sink].function),
        reason: "module graph maps its file below the denied prefix".to_owned()
    });
    trace.push(TraceStep {
        fact:   format!("fn `{}` lives under `{from}`", fact.function),
        reason: "module graph maps its file below the constrained prefix".to_owned()
    });
    let direct = chain.len() == 2;
    let how = if direct {
        "calls"
    } else {
        "transitively reaches"
    };
    let detail = format!(
        "invariant violated: `{}` ({from}) {how} `{}` ({to}); the layer boundary forbids it",
        fact.function, facts[sink].function
    );
    Some((detail, trace))
}
