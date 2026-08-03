// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Construction of [`TypeNode`]s from data definitions: structures and
//! enums, with their fields, derives, and source anchor.

use rustmanifest_schema::Range;

use crate::semantic::graph::nodes::{FieldNode, TypeKind, TypeNode};

pub(in crate::semantic::graph) fn node(source: usize, definition: &syn::ItemStruct) -> TypeNode {
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
pub(in crate::semantic::graph) fn enum_node(
    source: usize,
    definition: &syn::ItemEnum
) -> TypeNode {
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
