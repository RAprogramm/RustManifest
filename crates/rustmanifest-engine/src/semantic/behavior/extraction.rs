// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Fact extraction of one function: seeds the collector from the
//! signature, the declaration context, and the workspace typing tables,
//! walks the body, then normalizes the accumulated observations into the
//! facts the function carries.

use rustmanifest_schema::Range;
use syn::visit::Visit;

use super::{
    collector::{FactCollector, name_census},
    declaration::ItemContext,
    folding::fold_constants,
    signature::{
        error_types, param_slots
    },
    tables::TypeTables,
    targeting::name_targets,
    types::{Effect, EffectSite, FunctionFacts}
};

/// Everything the reading of one body is handed besides the body itself:
/// the workspace typing tables, the imports of its file, and what a
/// resolver settled about that file.
///
/// One value rather than three parameters, because the three always
/// travel together and are never chosen apart.
pub(super) struct Known<'a> {
    /// Workspace typing tables.
    pub(super) tables:  &'a TypeTables,
    /// Imports of the file the body lives in.
    pub(super) imports: &'a std::collections::BTreeMap<String, String>,
    /// What a resolver settled about that file, by byte range.
    pub(super) settled: &'a super::assembly::Settled
}

/// Extracts the facts of one function body against its signature.
/// Records the effect an `unsafe` signature carries by its own word,
/// with the signature itself standing as the proving site.
fn mark_unsafe(collector: &mut FactCollector, signature: &syn::Signature) {
    if matches!(signature.safety, syn::Safety::Unsafe(_)) {
        collector.facts.effects.push(Effect::Unsafe);
        collector
            .facts
            .effect_sites
            .push(unsafe_signature_site(signature));
    }
}

/// What the function takes, as the compiler read its signature.
fn taken(
    known: &Known<'_>,
    bytes: &std::ops::Range<usize>
) -> Vec<super::types::ParamFacts> {
    let at = (
        u32::try_from(bytes.start).unwrap_or(u32::MAX),
        u32::try_from(bytes.end).unwrap_or(u32::MAX)
    );
    known.settled.params.get(&at).cloned().unwrap_or_default()
}

/// What the function hands back, as the compiler rendered its
/// signature.
fn handed_back(known: &Known<'_>, bytes: &std::ops::Range<usize>) -> Option<String> {
    let at = (
        u32::try_from(bytes.start).unwrap_or(u32::MAX),
        u32::try_from(bytes.end).unwrap_or(u32::MAX)
    );
    known.settled.returns.get(&at).cloned()
}

pub(super) fn function_facts(
    source: usize,
    function: String,
    context: &ItemContext,
    signature: &syn::Signature,
    block: &syn::Block,
    known: &Known<'_>
) -> FunctionFacts {
    let bytes = signature.ident.span().byte_range();
    let census = name_census(block);
    let params = taken(known, &bytes);
    let collected = params.clone();
    let mut collector = FactCollector {
        facts:            FunctionFacts {
            function,
            source,
            range: Range {
                start: u32::try_from(bytes.start).unwrap_or(u32::MAX),
                end:   u32::try_from(bytes.end).unwrap_or(u32::MAX)
            },
            reads: Vec::new(),
            writes: Vec::new(),
            builds: Vec::new(),
            constructs: Vec::new(),
            built_fields: Vec::new(),
            error_types: Vec::new(),
            effects: Vec::new(),
            effect_sites: Vec::new(),
            params,
            flags: context.flags,
            calls: Vec::new(),
            clones: 0,
            allocations: 0,
            call_sites: Vec::new(),
            slots: param_slots(signature),
            input_reads: Vec::new(),
            operations: Vec::new(),
            choices: Vec::new(),
            handbacks: Vec::new(),
            output_writes: Vec::new(),
            exits: Vec::new(),
            queries: Vec::new()
        },
        param_types:      collected
            .iter()
            .map(|param| (param.name.clone(), param.type_name.clone()))
            .collect(),
        settled:          known.settled.produced.clone(),
        reached:          known.settled.reached.clone(),
        receivers:        known.settled.receivers.clone(),
        takes:            known.settled.takes.clone(),
        held:             known.settled.fields.clone(),
        typed:            known.settled.types.clone(),
        rendered:         known.settled.rendered.clone(),
        leaving:          known
            .settled
            .rendered
            .iter()
            .filter(|(_, rendered)| rendered.trim() == "!")
            .map(|(at, _)| *at)
            .collect(),
        items:            known.settled.items.clone(),
        named:            known.settled.named.clone(),
        local_wraps:      std::collections::BTreeMap::new(),
        imports:          known.imports.clone(),
        fresh:            std::collections::BTreeSet::new(),
        buffered_streams: std::collections::BTreeMap::new(),
        guards:           Vec::new(),
        prior_exits:      0,
        dead:             false,
        slots:            param_slots(signature),
        origins:          std::collections::BTreeMap::new(),
        written:          census.written,
        sole:             census.sole,
        produced:         std::collections::BTreeMap::new(),
        conditions:       Vec::new()
    };
    mark_unsafe(&mut collector, signature);
    collector.facts.error_types = error_types(handed_back(known, &bytes).as_deref());
    collector.visit_block(block);
    collector.collect_tail(block);
    collector.facts.reads.sort();
    collector.facts.reads.dedup();
    collector.facts.writes.sort();
    collector.facts.writes.dedup();
    collector.facts.constructs.sort();
    collector.facts.constructs.dedup();
    collector.facts.effects.sort();
    collector.facts.effects.dedup();
    collector.facts.effect_sites.sort_by(|left, right| {
        left.effect
            .cmp(&right.effect)
            .then(left.range.start.cmp(&right.range.start))
            .then(left.proof.cmp(&right.proof))
    });
    collector.facts.effect_sites.dedup();
    collector.facts.calls.sort();
    collector.facts.calls.dedup();
    name_targets(&mut collector.facts, handed_back(known, &bytes).as_deref());
    fold_constants(
        &mut collector.facts,
        block,
        &known.tables.constants,
        source,
        &known.tables.crates
    );
    collector.facts
}

/// Cites an `unsafe fn` signature as what proved the unsafe effect: the
/// keyword stands in the declaration, not in any call the body makes.
fn unsafe_signature_site(signature: &syn::Signature) -> EffectSite {
    let bytes = signature.ident.span().byte_range();
    EffectSite {
        effect: Effect::Unsafe,
        proof:  "unsafe fn".to_owned(),
        range:  Range {
            start: u32::try_from(bytes.start).unwrap_or(u32::MAX),
            end:   u32::try_from(bytes.end).unwrap_or(u32::MAX)
        }
    }
}
