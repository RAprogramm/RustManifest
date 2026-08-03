// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Trait default bodies: a trait method that carries a body in the
//! definition itself has facts of its own, qualified by the trait's
//! name; a method left for implementors carries none.

use super::{
    declaration::{ItemContext, item_flags},
    extraction::function_facts,
    signature::generics_bounds,
    tables::TypeTables,
    types::FunctionFacts
};

/// Facts of a trait definition's default-bodied methods, qualified by
/// the trait's own name.
pub(super) fn trait_default_facts(
    definition: &syn::ItemTrait,
    index: usize,
    tables: &TypeTables,
    imports: &std::collections::BTreeMap<String, String>
) -> Vec<FunctionFacts> {
    let trait_name = definition.ident.to_string();
    let public = matches!(definition.vis, syn::Visibility::Public(_));
    let trait_bounds = generics_bounds(&definition.generics);
    let mut facts = Vec::new();
    for trait_item in &definition.items {
        let syn::TraitItem::Fn(method) = trait_item else {
            continue;
        };
        let Some(block) = &method.default else {
            continue;
        };
        facts.push(function_facts(
            index,
            format!("{trait_name}::{}", method.sig.ident),
            &ItemContext {
                impl_type: Some(&trait_name),
                flags:     item_flags(true, &method.attrs, public, false),
                bounds:    trait_bounds.clone()
            },
            &method.sig,
            block,
            tables,
            imports
        ));
    }
    facts
}
