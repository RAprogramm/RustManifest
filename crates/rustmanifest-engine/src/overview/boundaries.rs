// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Outside-world boundaries: the functions whose effects leave the
//! process, listed with the effects that put them there.

use super::fragment::truncation_note;
use crate::{
    semantic::{Effect, FunctionFacts},
    source::Source
};

/// Effects that mark a function as an outside-world boundary.
const BOUNDARY_EFFECTS: [Effect; 5] = [
    Effect::FileSystem,
    Effect::Network,
    Effect::Process,
    Effect::Environment,
    Effect::ConsoleIo
];

/// Prints functions that touch the world outside the process; a name
/// shared by several functions is qualified by its file.
pub(super) fn boundary_section(
    out: &mut String,
    sources: &[Source],
    facts: &[FunctionFacts],
    top: usize
) {
    out.push_str("## Outside-world boundaries\n\n");
    let touching: Vec<&FunctionFacts> = facts
        .iter()
        .filter(|fact| {
            fact.effects
                .iter()
                .any(|effect| BOUNDARY_EFFECTS.contains(effect))
        })
        .collect();
    let boundaries: Vec<String> = touching
        .iter()
        .map(|fact| {
            let effects: Vec<String> = fact
                .effects
                .iter()
                .filter(|effect| BOUNDARY_EFFECTS.contains(effect))
                .map(ToString::to_string)
                .collect();
            let duplicated = touching
                .iter()
                .filter(|other| other.function == fact.function)
                .count()
                > 1;
            let qualifier = if duplicated {
                sources
                    .get(fact.source)
                    .map_or_else(String::new, |source| format!(" ({})", source.short_path()))
            } else {
                String::new()
            };
            format!("- `{}`{qualifier}: {}\n", fact.function, effects.join(", "))
        })
        .collect();
    if boundaries.is_empty() {
        out.push_str("- none observed\n\n");
        return;
    }
    for line in boundaries.iter().take(top) {
        out.push_str(line);
    }
    truncation_note(out, boundaries.len(), top);
    out.push('\n');
}
