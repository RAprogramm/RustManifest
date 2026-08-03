// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Fact extraction of one function: seeds the collector from the
//! signature, the declaration context, and the workspace typing tables,
//! walks the body, then normalizes the accumulated observations into the
//! facts the function carries.

use rustmanifest_schema::Range;
use syn::visit::Visit;

use super::{
    collector::FactCollector,
    declaration::ItemContext,
    signature::{error_types, param_facts, parameter_elements, parameter_types},
    tables::TypeTables,
    types::{Effect, FunctionFacts}
};

/// Extracts the facts of one function body against its signature.
pub(super) fn function_facts(
    source: usize,
    function: String,
    context: &ItemContext<'_>,
    signature: &syn::Signature,
    block: &syn::Block,
    tables: &TypeTables,
    imports: &std::collections::BTreeMap<String, String>
) -> FunctionFacts {
    let bytes = signature.ident.span().byte_range();
    let params = param_facts(signature, context.impl_type);
    let mut collector = FactCollector {
        facts:       FunctionFacts {
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
            error_types: Vec::new(),
            effects: Vec::new(),
            params,
            flags: context.flags,
            calls: Vec::new(),
            clones: 0,
            allocations: 0,
            call_sites: Vec::new(),
            output_writes: Vec::new()
        },
        param_types: parameter_types(signature, context.impl_type),
        elem_types:  parameter_elements(signature),
        fn_returns:  tables.returns.clone(),
        field_types: tables.fields.clone(),
        field_elems: tables.field_elems.clone(),
        field_wraps: tables.field_wraps.clone(),
        crates:      tables.crates.clone(),
        src:         source,
        bounds:      context.bounds.clone(),
        local_wraps: std::collections::BTreeMap::new(),
        imports:     imports.clone(),
        fresh:       std::collections::BTreeSet::new(),
        guards:      Vec::new(),
        prior_exits: 0,
        dead:        false
    };
    if matches!(signature.safety, syn::Safety::Unsafe(_)) {
        collector.facts.effects.push(Effect::Unsafe);
    }
    collector.facts.error_types = error_types(signature);
    collector.visit_block(block);
    collector.facts.reads.sort();
    collector.facts.reads.dedup();
    collector.facts.writes.sort();
    collector.facts.writes.dedup();
    collector.facts.constructs.sort();
    collector.facts.constructs.dedup();
    collector.facts.effects.sort();
    collector.facts.effects.dedup();
    collector.facts.calls.sort();
    collector.facts.calls.dedup();
    collector.facts
}
