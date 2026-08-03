// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Construction of [`ImplNode`]s from impl blocks, including the type
//! references each method body mentions.

use std::collections::BTreeSet;

use syn::visit::Visit;

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
            Some(MethodNode {
                name:      function.sig.ident.to_string(),
                type_refs: body_type_refs(&function.block)
            })
        })
        .collect();
    Some(ImplNode {
        type_name,
        trait_name,
        source,
        derived: false,
        methods
    })
}

/// Collects the first segments of qualified paths in a method body that
/// look like type names (leading uppercase), deduplicated and sorted.
fn body_type_refs(block: &syn::Block) -> Vec<String> {
    let mut collector = TypeRefCollector::default();
    collector.visit_block(block);
    collector.refs.into_iter().collect()
}

/// Syntax visitor gathering type-looking path heads from expressions.
#[derive(Default)]
struct TypeRefCollector {
    /// Distinct type-looking path heads seen so far.
    refs: BTreeSet<String>
}

impl<'ast> Visit<'ast> for TypeRefCollector {
    fn visit_path(&mut self, path: &'ast syn::Path) {
        if path.segments.len() >= 2
            && let Some(first) = path.segments.first()
        {
            let head = first.ident.to_string();
            if head.chars().next().is_some_and(char::is_uppercase) {
                self.refs.insert(head);
            }
        }
        syn::visit::visit_path(self, path);
    }
}
