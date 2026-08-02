// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Construction of graph nodes from `syn` items: one function per item
//! kind. Supporting a new item kind means a new constructor here and one
//! match arm at the call sites.

use std::collections::BTreeSet;

use rustmanifest_schema::Range;
use syn::visit::Visit;

use super::nodes::{FieldNode, ImplNode, MethodNode, TraitNode, TypeKind, TypeNode};

pub(super) fn node(source: usize, definition: &syn::ItemStruct) -> TypeNode {
    let bytes = definition.ident.span().byte_range();
    let fields = match &definition.fields {
        syn::Fields::Named(named) => named
            .named
            .iter()
            .filter_map(|field| {
                field.ident.as_ref().map(|ident| FieldNode {
                    name:      ident.to_string(),
                    type_name: last_segment(&field.ty)
                })
            })
            .collect(),
        _ => Vec::new()
    };
    TypeNode {
        name: definition.ident.to_string(),
        source,
        range: byte_range(&bytes),
        kind: TypeKind::Struct,
        generated: false,
        derives: derive_names(&definition.attrs),
        fields
    }
}

/// Lists the trait names of every `#[derive(...)]` attribute.
fn derive_names(attrs: &[syn::Attribute]) -> Vec<String> {
    let mut names = Vec::new();
    for attribute in attrs {
        if !attribute.path().is_ident("derive") {
            continue;
        }
        let parsed = attribute.parse_args_with(
            syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated
        );
        if let Ok(paths) = parsed {
            for path in paths {
                if let Some(segment) = path.segments.last() {
                    names.push(segment.ident.to_string());
                }
            }
        }
    }
    names
}

/// Converts a span byte range into the schema [`Range`].
fn byte_range(bytes: &std::ops::Range<usize>) -> Range {
    Range {
        start: u32::try_from(bytes.start).unwrap_or(u32::MAX),
        end:   u32::try_from(bytes.end).unwrap_or(u32::MAX)
    }
}

/// Builds a [`TypeNode`] from a parsed enum definition; every variant
/// payload becomes one field entry.
pub(super) fn enum_node(source: usize, definition: &syn::ItemEnum) -> TypeNode {
    let bytes = definition.ident.span().byte_range();
    let mut fields = Vec::new();
    for variant in &definition.variants {
        for field in &variant.fields {
            let name = field.ident.as_ref().map_or_else(
                || variant.ident.to_string(),
                |ident| format!("{}.{ident}", variant.ident)
            );
            fields.push(FieldNode {
                name,
                type_name: last_segment(&field.ty)
            });
        }
    }
    TypeNode {
        name: definition.ident.to_string(),
        source,
        range: byte_range(&bytes),
        kind: TypeKind::Enum,
        generated: false,
        derives: derive_names(&definition.attrs),
        fields
    }
}

/// Builds an [`ImplNode`] from a parsed impl block, or [`None`] when the
/// self type is not a nameable path (`impl Trait for &T`, trait objects).
pub(super) fn impl_node(source: usize, definition: &syn::ItemImpl) -> Option<ImplNode> {
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

/// Builds a [`TraitNode`] from a parsed trait definition.
pub(super) fn trait_node(source: usize, definition: &syn::ItemTrait) -> TraitNode {
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

/// Renders the last path segment of a type, or a placeholder for non-path
/// types such as references, tuples, and slices.
fn last_segment(ty: &syn::Type) -> String {
    match ty {
        syn::Type::Path(path) => path
            .path
            .segments
            .last()
            .map_or_else(|| "<type>".to_owned(), |segment| segment.ident.to_string()),
        syn::Type::Reference(reference) => last_segment(&reference.elem),
        _ => "<type>".to_owned()
    }
}
