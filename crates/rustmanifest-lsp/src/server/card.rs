// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The fact cards for the reader's cursor, in the visual language
//! editors already speak: the declaration in a highlighted code fence,
//! sections divided by rules, section names in bold.
//!
//! Everything on a card is a recorded fact of the buffer's text and
//! nothing else: what a function touches by memory, where it writes
//! and through what buffering, how each parameter takes its value,
//! what effects its own body proves and what it reaches through calls;
//! what a type holds, implements, and weighs. A
//! section with nothing recorded is absent, so a short card is a
//! smaller claim rather than a vaguer one. The declarations in the
//! fences are reconstructed from the facts — parameter modes, field
//! types — never sliced blindly, so the fence shows exactly what the
//! facts carry.

mod output;

use rustmanifest_engine::{FunctionFacts, ParamMode, semantic::Effect};

/// The markdown card of one function's recorded facts.
pub(super) fn card(fact: &FunctionFacts, reached: &[Effect]) -> String {
    let mut lines = vec![format!(
        "```rust\nfn {}({})\n```",
        fact.function,
        fact.params
            .iter()
            .map(spelled)
            .collect::<Vec<_>>()
            .join(", ")
    )];
    let mut memory = Vec::new();
    if fact.clones > 0 {
        memory.push(format!("clones: {}", fact.clones));
    }
    if fact.allocations > 0 {
        memory.push(format!("allocations: {}", fact.allocations));
    }
    if !memory.is_empty() {
        lines.push("---".to_owned());
        lines.push(format!("**memory** — {}", memory.join(" · ")));
    }
    if let Some(spoken) = output::output_section(&fact.output_writes) {
        lines.push("---".to_owned());
        lines.push(spoken);
    }
    if !fact.params.is_empty() {
        lines.push("---".to_owned());
        let mut told = vec!["**parameters**".to_owned()];
        for param in &fact.params {
            told.push(format!(
                "- `{}: {}` — taken {}, used {} time(s){}",
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
        lines.push(told.join("\n"));
    }
    let mut effects = Vec::new();
    if !fact.effects.is_empty() {
        let owned: Vec<String> = fact
            .effects
            .iter()
            .map(|effect| format!("`{effect}`"))
            .collect();
        effects.push(format!("own body: {}", owned.join(", ")));
    }
    let inherited: Vec<String> = reached
        .iter()
        .filter(|effect| !fact.effects.contains(effect))
        .map(|effect| format!("`{effect}`"))
        .collect();
    if !inherited.is_empty() {
        effects.push(format!("reached through calls: {}", inherited.join(", ")));
    }
    if !effects.is_empty() {
        lines.push("---".to_owned());
        lines.push(format!("**effects** — {}", effects.join(" · ")));
    }
    if !fact.call_sites.is_empty() {
        lines.push("---".to_owned());
        lines.push(format!("**call sites written** {}", fact.call_sites.len()));
    }
    lines.join("\n\n")
}

/// One parameter as the declaration spells it, mode included.
fn spelled(param: &rustmanifest_engine::ParamFacts) -> String {
    if param.name == "self" {
        return match param.mode {
            ParamMode::Owned => "self".to_owned(),
            ParamMode::Borrowed => "&self".to_owned(),
            ParamMode::BorrowedMut => "&mut self".to_owned()
        };
    }
    match param.mode {
        ParamMode::Owned => format!("{}: {}", param.name, param.type_name),
        ParamMode::Borrowed => format!("{}: &{}", param.name, param.type_name),
        ParamMode::BorrowedMut => format!("{}: &mut {}", param.name, param.type_name)
    }
}

/// The inference section of one inherited effect: the numbered steps
/// from the function to the written call that proves the effect.
pub(super) fn why_section(effect: &str, answer: &rustmanifest_engine::semantic::Answer) -> String {
    let mut lines = vec![format!("---\n\n**why `{effect}`**")];
    for (position, step) in answer.steps.iter().enumerate() {
        lines.push(format!("{}. {} — {}", position + 1, step.fact, step.reason));
    }
    lines.push(format!("⇒ {}", answer.conclusion));
    lines.join("\n")
}

/// The fact card of one type: what it is, what it holds, what it
/// implements, and what a value of it weighs where that is derivable.
pub(super) fn type_card(
    types: &rustmanifest_engine::TypeGraph,
    node: &rustmanifest_engine::TypeNode,
    implemented: &[&str],
    shape: Option<&rustmanifest_engine::semantic::TypeShape>,
    oracle: Option<&super::snapshot::Indexed>,
    settled: Option<&rustmanifest_schema::Resolution>
) -> String {
    let kind = match node.kind {
        rustmanifest_engine::TypeKind::Struct => "struct",
        rustmanifest_engine::TypeKind::Enum => "enum"
    };
    let mut fence = vec![format!("```rust\n{kind} {} {{", node.name)];
    for field in &node.fields {
        fence.push(format!("    {}: {},", field.name, field.type_name));
    }
    fence.push("}\n```".to_owned());
    let mut lines = vec![fence.join("\n")];
    lines.push("---".to_owned());
    lines.push(traits_section(types, node, implemented, oracle, settled));
    if let Some(shape) = shape {
        let mut memory = Vec::new();
        if let Some(payload) = shape.payload {
            memory.push(format!(
                "- the value itself takes at least {payload} byte(s) on the stack — that is \
                 what every move or copy of it touches, whatever the contents are"
            ));
        }
        if shape.indirections > 0 {
            memory.push(format!(
                "- {} of its field(s) own heap data: the contents live in the heap, sized at \
                 runtime, and are freed when the value drops",
                shape.indirections
            ));
        }
        if !shape.unresolved.is_empty() {
            memory.push(format!(
                "- {} field type(s) the facts cannot size",
                shape.unresolved.len()
            ));
        }
        if !memory.is_empty() {
            lines.push("---".to_owned());
            lines.push(format!("**memory**\n{}", memory.join("\n")));
        }
    }
    lines.join("\n\n")
}

/// Traits the workspace's canon asks about first, in asking order.
const ASKED_TRAITS: [&str; 8] = [
    "Copy",
    "Clone",
    "Debug",
    "Default",
    "Display",
    "PartialEq",
    "Eq",
    "Hash"
];

/// The per-trait verdicts of one type: yes with the ground, no with
/// the proof where one exists, "not written" where absence is all the
/// facts can say — and the compiler's own territory named as such.
fn traits_section(
    types: &rustmanifest_engine::TypeGraph,
    node: &rustmanifest_engine::TypeNode,
    implemented: &[&str],
    oracle: Option<&super::snapshot::Indexed>,
    settled: Option<&rustmanifest_schema::Resolution>
) -> String {
    let mut told = vec!["**traits**".to_owned()];
    let mut named: Vec<&str> = ASKED_TRAITS.to_vec();
    for extra in implemented.iter().copied() {
        if !named.contains(&extra) {
            named.push(extra);
        }
    }
    let asked = oracle.filter(|held| held.oracle.knows(&node.name));
    for trait_name in named {
        told.push(format!(
            "- {}",
            verdict(types, node, implemented, asked, settled, trait_name)
        ));
    }
    match asked {
        Some(index) => {
            for auto in ["Send", "Sync"] {
                told.push(format!("- {}", indexed(index, &node.name, auto)));
            }
        }
        None => told.push(
            "- auto traits (`Send`, `Sync`) and blanket impls are the compiler's answer — \
             build its index (`cargo rustdoc -- --output-format json`) and the card will \
             carry them"
                .to_owned()
        )
    }
    told.join("\n")
}

/// One trait's verdict straight from the compiler's index, its age
/// named where a source is younger than the emission: degraded by
/// words, never silenced.
fn indexed(index: &super::snapshot::Indexed, type_name: &str, trait_name: &str) -> String {
    use rustmanifest_engine::semantic::OracleProvenance;
    let aged = if index.stale {
        ", index older than the sources"
    } else {
        ""
    };
    index.oracle.implements(type_name, trait_name).map_or_else(
        || format!("✗ `{trait_name}` — the compiler's index lists no impl{aged}"),
        |held| {
            format!(
                "✓ `{trait_name}` — the compiler's index ({}{aged})",
                match held.provenance {
                    OracleProvenance::Auto => "auto impl",
                    OracleProvenance::Blanket => "blanket impl",
                    OracleProvenance::Written => "written impl"
                }
            )
        }
    )
}

/// One trait's verdict with its ground: the workspace facts answer
/// first, and the compiler's index answers what the text cannot.
fn verdict(
    types: &rustmanifest_engine::TypeGraph,
    node: &rustmanifest_engine::TypeNode,
    implemented: &[&str],
    asked: Option<&super::snapshot::Indexed>,
    settled: Option<&rustmanifest_schema::Resolution>,
    trait_name: &str
) -> String {
    if implemented.contains(&trait_name) {
        return format!("✓ `{trait_name}` — a written impl");
    }
    if types.ability_given(&node.name, trait_name) {
        return format!("✓ `{trait_name}` — the compiler says the type carries it");
    }
    if trait_name == "Copy"
        && let Some(owning) = node.fields.iter().find(|field| {
            rustmanifest_engine::semantic::copyness(types, settled, &field.type_name)
                == Some(false)
        })
    {
        return format!(
            "✗ `Copy` — not derived, and `{}: {}` owns heap data — a type holding it cannot be \
             `Copy`",
            owning.name, owning.type_name
        );
    }
    if let Some(index) = asked {
        return indexed(index, &node.name, trait_name);
    }
    if trait_name == "Copy" {
        return "– `Copy` — not derived".to_owned();
    }
    format!("– `{trait_name}` — not written in the workspace")
}

/// The ownership story of the written call under the cursor: one step
/// per handed value, closing with where each one ends up.
pub(super) fn handover_section(answer: &rustmanifest_engine::semantic::Answer) -> String {
    let mut lines = vec!["**ownership at this call**".to_owned()];
    for (position, step) in answer.steps.iter().enumerate() {
        lines.push(format!("{}. {} — {}", position + 1, step.fact, step.reason));
    }
    lines.push(format!("⇒ {}", answer.conclusion));
    lines.join("\n")
}
