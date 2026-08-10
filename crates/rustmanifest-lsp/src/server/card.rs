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

/// The fact card of one type: what it is, what it holds, what it
/// implements, and what a value of it weighs where that is derivable.
pub(super) fn type_card(
    node: &rustmanifest_engine::TypeNode,
    implemented: &[&str],
    shape: Option<&rustmanifest_engine::semantic::TypeShape>
) -> String {
    let mut lines = vec![format!("### `{}`", node.name)];
    let kind = match node.kind {
        rustmanifest_engine::TypeKind::Struct => "struct",
        rustmanifest_engine::TypeKind::Enum => "enum"
    };
    lines.push(format!(
        "{kind} with {} recorded field(s)",
        node.fields.len()
    ));
    for field in &node.fields {
        lines.push(format!("- `{}: {}`", field.name, field.type_name));
    }
    if !node.derives.is_empty() {
        let derived: Vec<String> = node
            .derives
            .iter()
            .map(|name| format!("`{name}`"))
            .collect();
        lines.push(format!("derives: {}", derived.join(", ")));
    }
    if !implemented.is_empty() {
        let written: Vec<String> = implemented.iter().map(|name| format!("`{name}`")).collect();
        lines.push(format!("written impls: {}", written.join(", ")));
    }
    if let Some(shape) = shape {
        let mut memory = Vec::new();
        if let Some(payload) = shape.payload {
            memory.push(format!("at least {payload} guaranteed field byte(s)"));
        }
        if shape.indirections > 0 {
            memory.push(format!("{} heap header(s)", shape.indirections));
        }
        if !shape.unresolved.is_empty() {
            memory.push(format!(
                "{} field(s) of unsized types",
                shape.unresolved.len()
            ));
        }
        if !memory.is_empty() {
            lines.push(format!("memory: {}", memory.join(", ")));
        }
    }
    lines.join("\n\n")
}

/// The ownership story of the written call under the cursor: one step
/// per handed value, closing with where each one ends up.
pub(super) fn handover_section(answer: &rustmanifest_engine::semantic::Answer) -> String {
    let mut lines = vec!["### ownership at this call".to_owned()];
    for (position, step) in answer.steps.iter().enumerate() {
        lines.push(format!("{}. {} — {}", position + 1, step.fact, step.reason));
    }
    lines.push(format!("=> {}", answer.conclusion));
    lines.join("\n")
}
