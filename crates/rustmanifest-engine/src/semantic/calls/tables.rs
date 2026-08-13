// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Name lookup tables of one behavioral fact slice, the material every
//! candidate is matched against.

use std::collections::BTreeMap;

use crate::semantic::behavior::FunctionFacts;

/// Resolution tables of one fact slice: qualified names, bare free
/// functions, and method names, each to the fact indices carrying them.
pub(super) type NameTables<'facts> = (
    BTreeMap<&'facts str, Vec<usize>>,
    BTreeMap<&'facts str, Vec<usize>>,
    BTreeMap<&'facts str, Vec<usize>>
);

/// Builds the resolution tables the candidate loop and the overlay
/// share.
pub(super) fn name_tables(facts: &[FunctionFacts]) -> NameTables<'_> {
    let mut by_qualified: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    let mut by_bare_free: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    let mut by_method: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (index, fact) in facts.iter().enumerate() {
        by_qualified.entry(&fact.function).or_default().push(index);
        match fact.function.rsplit_once("::") {
            None => by_bare_free.entry(&fact.function).or_default().push(index),
            Some((_, method)) => by_method.entry(method).or_default().push(index)
        }
    }
    (by_qualified, by_bare_free, by_method)
}
