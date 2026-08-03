// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Construction of [`TraitNode`]s from trait definitions: the declared
//! method names and the source the trait comes from.

use crate::semantic::graph::nodes::TraitNode;

/// Builds a [`TraitNode`] from a parsed trait definition.
pub(in crate::semantic::graph) fn trait_node(
    source: usize,
    definition: &syn::ItemTrait
) -> TraitNode {
    let methods = definition
        .items
        .iter()
        .filter_map(|item| {
            let syn::TraitItem::Fn(function) = item else {
                return None;
            };
            Some(function.sig.ident.to_string())
        })
        .collect();
    TraitNode {
        name: definition.ident.to_string(),
        source,
        methods
    }
}
