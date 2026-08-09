// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The fact card of one function, for the reader's cursor.
//!
//! Everything on the card is a recorded fact of the buffer's text and
//! nothing else: what the function touches by memory — clones,
//! allocations, how each parameter takes its value — what effects its
//! own body proves and what it reaches through calls, and how many
//! call sites it writes. A section with nothing recorded is absent, so
//! a short card is a smaller claim rather than a vaguer one.

use rustmanifest_engine::{FunctionFacts, semantic::Effect};

/// The markdown card of one function's recorded facts.
pub(super) fn card(fact: &FunctionFacts, reached: &[Effect]) -> String {
    let mut lines = vec![format!("### `{}`", fact.function)];
    let mut memory = Vec::new();
    if fact.clones > 0 {
        memory.push(format!("clones: {}", fact.clones));
    }
    if fact.allocations > 0 {
        memory.push(format!("allocations: {}", fact.allocations));
    }
    if !memory.is_empty() {
        lines.push(format!("memory: {}", memory.join(", ")));
    }
    for param in &fact.params {
        lines.push(format!(
            "- `{}: {}` taken {}, used {} time(s){}",
            param.name,
            param.type_name,
            param.mode,
            param.uses,
            if param.field_reads > 0 {
                format!(", {} field read(s)", param.field_reads)
            } else {
                String::new()
            }
        ));
    }
    if !fact.effects.is_empty() {
        let owned: Vec<String> = fact
            .effects
            .iter()
            .map(|effect| format!("`{effect}`"))
            .collect();
        lines.push(format!("effects of its own body: {}", owned.join(", ")));
    }
    let inherited: Vec<String> = reached
        .iter()
        .filter(|effect| !fact.effects.contains(effect))
        .map(|effect| format!("`{effect}`"))
        .collect();
    if !inherited.is_empty() {
        lines.push(format!(
            "effects reached through calls: {}",
            inherited.join(", ")
        ));
    }
    if !fact.call_sites.is_empty() {
        lines.push(format!("call sites written: {}", fact.call_sites.len()));
    }
    lines.join("\n\n")
}

/// The inference section of one inherited effect: the numbered steps
/// from the function to the written call that proves the effect.
pub(super) fn why_section(effect: &str, answer: &rustmanifest_engine::semantic::Answer) -> String {
    let mut lines = vec![format!("why `{effect}`:")];
    for (position, step) in answer.steps.iter().enumerate() {
        lines.push(format!("{}. {} — {}", position + 1, step.fact, step.reason));
    }
    lines.push(format!("=> {}", answer.conclusion));
    lines.join("\n")
}
