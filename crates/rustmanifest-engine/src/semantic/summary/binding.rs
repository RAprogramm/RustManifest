// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Which one function a recorded call candidate can name.
//!
//! One rule, used by the grading walk and by the composition tier alike,
//! so the two can never disagree about what a candidate names and a trace
//! can say which rule it used.
//!
//! It is deliberately stricter than the call graph's own edge building: a
//! summary crediting a caller with a callee bound by module-qualifier
//! fallback or by an extension-trait default would state more than the
//! site proves.

use std::collections::{BTreeMap, BTreeSet};

use crate::semantic::behavior::FunctionFacts;

/// Index of every function whose name no other function shares, so a
/// candidate spelling one of these names cannot mean a second function.
pub(in crate::semantic) fn sole_by_name(facts: &[FunctionFacts]) -> BTreeMap<&str, usize> {
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
pub(in crate::semantic) fn bound_callee(
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
