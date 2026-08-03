// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The most contended state: fields ranked by how many distinct functions
//! write to them. Writes whose target stayed unresolved are left out
//! rather than guessed at.

use std::collections::BTreeMap;

use super::fragment::{push, truncation_note};
use crate::semantic::FunctionFacts;

/// Prints the most written state fields and their writer counts.
pub(super) fn state_section(out: &mut String, facts: &[FunctionFacts], top: usize) {
    out.push_str("## Most contended state\n\n");
    let mut writers: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for fact in facts {
        for write in &fact.writes {
            if !write.starts_with("?.") {
                writers.entry(write).or_default().push(&fact.function);
            }
        }
    }
    let mut ranked: Vec<(&str, Vec<&str>)> = writers.into_iter().collect();
    ranked.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(b.0)));
    if ranked.is_empty() {
        out.push_str("- no resolved field writes observed\n\n");
        return;
    }
    for (field, mut names) in ranked.iter().take(top).map(|(f, n)| (*f, n.clone())) {
        names.sort_unstable();
        names.dedup();
        push(
            out,
            &format!(
                "- `{field}`: {} writer(s): {}\n",
                names.len(),
                names.join(", ")
            )
        );
    }
    truncation_note(out, ranked.len(), top);
    out.push('\n');
}
