// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Derive-generated conversions: the `Error` derive guarantees an
//! `impl From<Source> for Owner` for every `#[from]` field, so those
//! functions carry facts even though no syntax carries their body.

use rustmanifest_schema::Range;

use super::{
    signature::type_last_segment,
    types::{FunctionFacts, FunctionFlags, ParamFacts, ParamMode}
};

/// Synthesized `From` conversion facts of one item: the `Error` derive
/// guarantees `impl From<Source> for Owner` for every `#[from]` field.
pub(super) fn error_conversions(item: &syn::Item, index: usize) -> Vec<FunctionFacts> {
    let mut facts = Vec::new();
    match item {
        syn::Item::Enum(definition) if derives_error(&definition.attrs) => {
            let owner = definition.ident.to_string();
            for variant in &definition.variants {
                for field in &variant.fields {
                    if from_attributed(&field.attrs)
                        && let Some(source_type) = type_last_segment(&field.ty)
                    {
                        facts.push(conversion_fact(index, &owner, source_type));
                    }
                }
            }
        }
        syn::Item::Struct(definition) if derives_error(&definition.attrs) => {
            let owner = definition.ident.to_string();
            for field in &definition.fields {
                if from_attributed(&field.attrs)
                    && let Some(source_type) = type_last_segment(&field.ty)
                {
                    facts.push(conversion_fact(index, &owner, source_type));
                }
            }
        }
        _ => {}
    }
    facts
}

/// Returns whether a `#[derive(...)]` list names the `Error` derive —
/// the thiserror pattern whose `#[from]` fields generate `From` impls.
fn derives_error(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("derive") {
            return false;
        }
        let mut named = false;
        let parsed = attr.parse_nested_meta(|meta| {
            if meta
                .path
                .segments
                .last()
                .is_some_and(|s| s.ident == "Error")
            {
                named = true;
            }
            Ok(())
        });
        parsed.is_ok() && named
    })
}

/// Returns whether a field carries the `#[from]` attribute.
fn from_attributed(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident("from"))
}

/// Synthesizes the fact of a derive-generated `From` conversion: the
/// `Error` derive guarantees `impl From<Source> for Owner` for every
/// `#[from]` field, so the generated function exists with exactly this
/// shape even though no syntax carries its body.
fn conversion_fact(source: usize, owner: &str, source_type: String) -> FunctionFacts {
    FunctionFacts {
        function: format!("{owner}::from"),
        source,
        range: Range {
            start: 0, end: 0
        },
        reads: Vec::new(),
        writes: Vec::new(),
        builds: Vec::new(),
        constructs: vec![owner.to_owned()],
        error_types: Vec::new(),
        effects: Vec::new(),
        params: vec![ParamFacts {
            name:        "source".to_owned(),
            type_name:   source_type,
            mode:        ParamMode::Owned,
            uses:        1,
            field_reads: 0
        }],
        flags: FunctionFlags::from_bits(FunctionFlags::TRAIT_IMPL | FunctionFlags::PUBLIC),
        calls: Vec::new(),
        clones: 0,
        allocations: 0,
        call_sites: Vec::new(),
        output_writes: Vec::new()
    }
}
