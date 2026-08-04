// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! How certainly the call graph carries an entry point to each function.
//!
//! Grading walks the graph outward from the entry and keeps, for every
//! function, the strongest grade any path gives it. The strength of a
//! single edge is decided by the call sites that provably bind to the
//! callee, so a callee reached only through sites that can never run is
//! itself dead, and a callee whose binding cannot be proven keeps the
//! weakest honest grade instead of a guess.

use std::collections::{BTreeMap, BTreeSet};

use crate::semantic::{
    behavior::{FunctionFacts, Grade},
    calls::CallGraph,
    summary::{bound_callee, sole_by_name}
};

/// Grades how certainly every function is reached from `entry`.
///
/// The entry itself is certain. A callee is graded by the weaker of its
/// caller's grade and the grade of the edge between them, and keeps the
/// strongest grade any path gives it. An edge is possible when the
/// dispatch behind it names several implementors, otherwise it is graded
/// by the call sites that provably bind to that callee. A function no
/// path grades stays [`Grade::Unreached`], which is a statement about
/// the call graph and not about the function.
pub(super) fn reachability(
    facts: &[FunctionFacts],
    graph: &CallGraph,
    qualifiers: &[BTreeSet<String>],
    entry: usize
) -> Vec<Grade> {
    if entry >= facts.len() {
        return vec![Grade::Unreached; facts.len()];
    }
    let bound = bound_sites(facts, qualifiers);
    let mut grades: Vec<Option<Grade>> = vec![None; facts.len()];
    grades[entry] = Some(Grade::Certain);
    let mut changed = true;
    while changed {
        changed = false;
        for caller in 0..facts.len() {
            let Some(reached) = grades[caller] else {
                continue;
            };
            for &callee in graph.callees(caller) {
                let edge = if graph.possibilistic(caller, callee) {
                    Grade::Possible
                } else {
                    bound[caller]
                        .get(&callee)
                        .copied()
                        .unwrap_or(Grade::Possible)
                };
                let candidate = reached.min(edge);
                if grades[callee].is_none_or(|known| candidate > known) {
                    grades[callee] = Some(candidate);
                    changed = true;
                }
            }
        }
    }
    grades
        .into_iter()
        .map(|grade| grade.unwrap_or(Grade::Unreached))
        .collect()
}

/// Grades every caller's edges by the call sites that provably bind to
/// them: caller index to callee index to the strongest grade among the
/// binding sites.
///
/// A callee reached only through sites that can never run is itself
/// dead; a callee no site binds to is absent from the map, and its edge
/// keeps the weakest honest grade — the graph proved the call, the
/// sites did not prove when it runs.
fn bound_sites(
    facts: &[FunctionFacts],
    qualifiers: &[BTreeSet<String>]
) -> Vec<BTreeMap<usize, Grade>> {
    let sole = sole_by_name(facts);
    facts
        .iter()
        .map(|fact| {
            let mut grades: BTreeMap<usize, Grade> = BTreeMap::new();
            for site in &fact.call_sites {
                let Some(callee) = bound_callee(&sole, qualifiers, &site.candidate) else {
                    continue;
                };
                let grade = site.guard.local_grade();
                grades
                    .entry(callee)
                    .and_modify(|known| *known = (*known).max(grade))
                    .or_insert(grade);
            }
            grades
        })
        .collect()
}
