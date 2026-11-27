// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Construction of [`ImplNode`]s from impl blocks, including the type
//! references each method body mentions.

use crate::semantic::graph::nodes::{ImplNode, MethodNode};

/// Builds an [`ImplNode`] from a parsed impl block, or [`None`] when the
/// self type is not a nameable path (`impl Trait for &T`, trait objects).
pub(in crate::semantic::graph) fn impl_node(
    source: usize,
    definition: &syn::ItemImpl
) -> Option<ImplNode> {
    let syn::Type::Path(self_type) = definition.self_ty.as_ref() else {
        return None;
    };
    let type_name = self_type.path.segments.last()?.ident.to_string();
    let trait_name = definition
        .trait_
        .as_ref()
        .and_then(|(path, _)| path.segments.last())
        .map(|segment| segment.ident.to_string());
    let methods = definition
        .items
        .iter()
        .filter_map(|item| {
            let syn::ImplItem::Fn(function) = item else {
                return None;
            };
            let span = {
                use syn::spanned::Spanned as _;

                function.block.span().byte_range()
            };
            Some(MethodNode {
                name:      function.sig.ident.to_string(),
                type_refs: Vec::new(),
                body:      rustmanifest_schema::Range {
                    start: u32::try_from(span.start).unwrap_or(u32::MAX),
                    end:   u32::try_from(span.end).unwrap_or(u32::MAX)
                }
            })
        })
        .collect();
    let whole = {
        use syn::spanned::Spanned as _;

        definition.span().byte_range()
    };
    Some(ImplNode {
        type_name,
        trait_name,
        header: rustmanifest_schema::Range {
            start: u32::try_from(whole.start).unwrap_or(u32::MAX),
            end:   u32::try_from(whole.end).unwrap_or(u32::MAX)
        },
        source,
        derived: false,
        methods
    })
}
