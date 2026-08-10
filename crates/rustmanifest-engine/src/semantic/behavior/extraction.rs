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
    signature::{error_types, param_facts, param_slots, parameter_elements, parameter_types},
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
pub(super) fn function_facts(
    source: usize,
    function: String,
    context: &ItemContext<'_>,
    signature: &syn::Signature,
    block: &syn::Block,
    known: &Known<'_>
) -> FunctionFacts {
    let bytes = signature.ident.span().byte_range();
    let census = name_census(block);
    let params = param_facts(signature, context.impl_type);
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
        param_types:      parameter_types(signature, context.impl_type),
        elem_types:       parameter_elements(signature),
        fn_returns:       known.tables.returns.clone(),
        mutates:          known.tables.mutates.clone(),
        field_types:      known.tables.fields.clone(),
        field_elems:      known.tables.field_elems.clone(),
        field_wraps:      known.tables.field_wraps.clone(),
        field_pairs:      known.tables.field_pairs.clone(),
        crates:           known.tables.crates.clone(),
        declared:         known.tables.declared.clone(),
        payloads:         known.tables.payloads.clone(),
        settled:          known.settled.produced.clone(),
        receivers:        known.settled.receivers.clone(),
        held:             known.settled.fields.clone(),
        src:              source,
        bounds:           context.bounds.clone(),
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
    if matches!(signature.safety, syn::Safety::Unsafe(_)) {
        collector.facts.effects.push(Effect::Unsafe);
        collector
            .facts
            .effect_sites
            .push(unsafe_signature_site(signature));
    }
    collector.facts.error_types = error_types(signature);
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
    name_targets(&mut collector.facts, signature, context.impl_type);
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
