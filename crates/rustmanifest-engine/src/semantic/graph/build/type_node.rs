// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Construction of [`TypeNode`]s from data definitions: structures and
//! enums, with their fields, derives, and source anchor.

use rustmanifest_schema::Range;

use crate::semantic::graph::nodes::{FieldNode, TypeKind, TypeNode};

pub(in crate::semantic::graph) fn node(
    source: usize,
    definition: &syn::ItemStruct,
    nested: bool
) -> TypeNode {
    let bytes = definition.ident.span().byte_range();
    let fields = match &definition.fields {
        syn::Fields::Named(named) => named
            .named
            .iter()
            .filter_map(|field| {
                field.ident.as_ref().map(|ident| FieldNode {
                    name:      ident.to_string(),
                    type_name: String::new(),
                    element:   None,
                    key:       None,
                    holds:     None
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
        nested,
        generic: !definition.generics.params.is_empty(),
        fields
    }
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
    definition: &syn::ItemEnum,
    nested: bool
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
                type_name: String::new(),
                element: None,
                key: None,
                holds: None
            });
        }
    }
    TypeNode {
        name: definition.ident.to_string(),
        source,
        range: byte_range(&bytes),
        kind: TypeKind::Enum,
        generated: false,
        nested,
        generic: !definition.generics.params.is_empty(),
        fields
    }
}

