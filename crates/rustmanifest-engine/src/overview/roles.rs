// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The behavioral role distribution: how many functions the engine placed
//! in each role, counted from their facts.

use std::collections::BTreeMap;

use super::fragment::push;
use crate::semantic::FunctionFacts;

/// Prints the behavioral role distribution.
pub(super) fn role_section(out: &mut String, facts: &[FunctionFacts]) {
    out.push_str("## Behavioral roles\n\n");
    let mut distribution: BTreeMap<String, usize> = BTreeMap::new();
    for fact in facts {
        *distribution
            .entry(crate::semantic::role(fact).to_string())
            .or_insert(0) += 1;
    }
    for (name, count) in &distribution {
        push(out, &format!("- {name}: {count}\n"));
    }
    out.push('\n');
}
