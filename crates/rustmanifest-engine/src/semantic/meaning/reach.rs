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
    calls::CallGraph
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

/// Index of every function whose name no other function shares, so a
/// candidate spelling one of these names cannot mean a second function.
fn sole_by_name(facts: &[FunctionFacts]) -> BTreeMap<&str, usize> {
    let mut seen: BTreeMap<&str, Option<usize>> = BTreeMap::new();
    for (index, fact) in facts.iter().enumerate() {
        seen.entry(fact.function.as_str())
            .and_modify(|slot| *slot = None)
            .or_insert(Some(index));
    }
    seen.into_iter()
        .filter_map(|(name, index)| index.map(|index| (name, index)))
        .collect()
}

/// The one function a recorded call candidate can name, or `None` when
/// the spelling admits another.
///
/// Two bindings are provable: the candidate is a function's own name and
/// no other function carries it, or the candidate is `module::name`,
/// exactly one function is called `name`, and `module` is the very scope
/// that declares it — the qualifier must own the callee, not merely
/// exist somewhere in the workspace. Any workspace module would do
/// otherwise, and `elsewhere::name` would certify a function that module
/// does not hold. A candidate whose receiver type is unknown
/// (`?::method`) names a method rather than a function and binds to
/// nothing — matching on the method name alone would bind whichever type
/// happened to declare it.
fn bound_callee(
    sole: &BTreeMap<&str, usize>,
    qualifiers: &[BTreeSet<String>],
    candidate: &str
) -> Option<usize> {
    if candidate.starts_with("?::") {
        return None;
    }
    if let Some(index) = sole.get(candidate) {
        return Some(*index);
    }
    let (qualifier, bare) = candidate.rsplit_once("::")?;
    let index = *sole.get(bare)?;
    qualifiers.get(index)?.contains(qualifier).then_some(index)
}
