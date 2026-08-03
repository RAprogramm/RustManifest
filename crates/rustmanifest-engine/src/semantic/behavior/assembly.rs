// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Fact assembly: builds the workspace typing tables once, then walks
//! every source's flattened items and turns each one into the facts it
//! carries — a free function, the methods of an impl block, the `From`
//! conversions an error derive generates, and a trait's default bodies.

use super::{
    conversion::error_conversions,
    declaration::{ItemContext, is_attributed, item_flags},
    extraction::function_facts,
    imports::use_imports,
    signature::generics_bounds,
    tables::TypeTables,
    trait_default::trait_default_facts,
    types::FunctionFacts
};
use crate::{semantic::expr::impl_type_name, source::Source};

/// Extracts behavioral facts for every function in `sources`; files that do
/// not parse as Rust contribute no facts.
#[must_use]
pub fn behavior(sources: &[Source]) -> Vec<FunctionFacts> {
    let tables = TypeTables::of(sources);
    let mut facts = Vec::new();
    for (index, source) in sources.iter().enumerate() {
        let Ok(file) = syn::parse_file(source.text()) else {
            continue;
        };
        let mut items = Vec::new();
        crate::ast::flattened(&file.items, &mut items);
        let imports = use_imports(&items);
        for item in &items {
            match item {
                syn::Item::Fn(function) => {
                    facts.push(function_facts(
                        index,
                        function.sig.ident.to_string(),
                        &ItemContext {
                            impl_type: None,
                            flags:     item_flags(
                                false,
                                &function.attrs,
                                matches!(function.vis, syn::Visibility::Public(_)),
                                false
                            ),
                            bounds:    std::collections::BTreeMap::new()
                        },
                        &function.sig,
                        &function.block,
                        &tables,
                        &imports
                    ));
                }
                syn::Item::Impl(block) => {
                    let Some(type_name) = impl_type_name(block) else {
                        continue;
                    };
                    let trait_impl = block.trait_.is_some();
                    let impl_attributed = is_attributed(&block.attrs);
                    let impl_bounds = generics_bounds(&block.generics);
                    for impl_item in &block.items {
                        let syn::ImplItem::Fn(method) = impl_item else {
                            continue;
                        };
                        facts.push(function_facts(
                            index,
                            format!("{type_name}::{}", method.sig.ident),
                            &ItemContext {
                                impl_type: Some(&type_name),
                                flags:     item_flags(
                                    trait_impl,
                                    &method.attrs,
                                    matches!(method.vis, syn::Visibility::Public(_)),
                                    impl_attributed
                                ),
                                bounds:    impl_bounds.clone()
                            },
                            &method.sig,
                            &method.block,
                            &tables,
                            &imports
                        ));
                    }
                }
                syn::Item::Enum(_) | syn::Item::Struct(_) => {
                    facts.extend(error_conversions(item, index));
                }
                syn::Item::Trait(definition) => {
                    facts.extend(trait_default_facts(definition, index, &tables, &imports));
                }
                _ => {}
            }
        }
    }
    facts
}
