// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Return-type table: every free function, every associated function
//! keyed `Type::name`, and every default-bodied trait method mapped to
//! what its signature declares it returns.

use crate::{
    semantic::{
        behavior::{
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
pub(super) fn receiver_mutability(sources: &[Source]) -> Scoped<bool> {
    let mut table = Scoped::default();
    for (index, source) in sources.iter().enumerate() {
        let Some(file) = source.tree() else {
            continue;
        };
        let mut items = Vec::new();
        crate::ast::flattened(&file.items, &mut items);
        for item in items {
            let syn::Item::Impl(block) = item else {
                continue;
            };
            let Some(owner) = impl_type_name(block) else {
                continue;
            };
            for impl_item in &block.items {
                let syn::ImplItem::Fn(method) = impl_item else {
                    continue;
                };
                let Some(syn::FnArg::Receiver(receiver)) = method.sig.inputs.first() else {
                    continue;
                };
                table.note(
                    format!("{owner}::{}", method.sig.ident),
                    index,
                    receiver.mutability.is_some()
                );
            }
        }
    }
    table
}

