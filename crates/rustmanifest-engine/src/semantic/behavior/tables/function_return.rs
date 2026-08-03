// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Return-type table: every free function, every associated function
//! keyed `Type::name`, and every default-bodied trait method mapped to
//! what its signature declares it returns.

use crate::{
    semantic::{
        behavior::{
            returned::{Returned, returned_of},
            scoped::Scoped
        },
        expr::impl_type_name
    },
    source::Source
};

/// Maps every uniquely named free function — and every associated
/// function keyed `Type::name` — to its return type; keys with
/// conflicting returns map to nothing, so typing through this table
/// never guesses.
pub(super) fn return_types(sources: &[Source]) -> Scoped<Returned> {
    let mut table = Scoped::default();
    for (index, source) in sources.iter().enumerate() {
        let Ok(file) = syn::parse_file(source.text()) else {
            continue;
        };
        let mut items = Vec::new();
        crate::ast::flattened(&file.items, &mut items);
        for item in items {
            match item {
                syn::Item::Fn(function) => {
                    record_return(
                        &mut table,
                        function.sig.ident.to_string(),
                        index,
                        None,
                        &function.sig
                    );
                }
                syn::Item::Impl(block) => {
                    let Some(owner) = impl_type_name(block) else {
                        continue;
                    };
                    for impl_item in &block.items {
                        if let syn::ImplItem::Fn(method) = impl_item {
                            record_return(
                                &mut table,
                                format!("{owner}::{}", method.sig.ident),
                                index,
                                Some(&owner),
                                &method.sig
                            );
                        }
                    }
                }
                syn::Item::Trait(definition) => {
                    let owner = definition.ident.to_string();
                    for trait_item in &definition.items {
                        if let syn::TraitItem::Fn(method) = trait_item
                            && method.default.is_some()
                        {
                            record_return(
                                &mut table,
                                format!("{owner}::{}", method.sig.ident),
                                index,
                                Some(&owner),
                                &method.sig
                            );
                        }
                    }
                }
                _ => {}
            }
        }
    }
    table
}

/// Records one function's return under `key` with its source.
fn record_return(
    table: &mut Scoped<Returned>,
    key: String,
    source: usize,
    owner: Option<&str>,
    signature: &syn::Signature
) {
    let Some(returned) = returned_of(signature, owner) else {
        return;
    };
    table.note(key, source, returned);
}
