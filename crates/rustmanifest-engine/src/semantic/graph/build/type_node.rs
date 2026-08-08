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
                    type_name: last_segment(&field.ty),
                    element:   held_element(&field.ty),
                    key:       looked_up_by(&field.ty)
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
                type_name: last_segment(&field.ty),
                element: held_element(&field.ty),
                key: looked_up_by(&field.ty)
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

/// The type standing under the guards written around it.
///
/// A lock, a cell, or a sharing pointer says how a value is reached —
/// one user at a time, checked at runtime, counted owners — and never
/// what the value is. A reading that asks what a field holds therefore
/// looks through them: the guarded type is the single written type
/// argument, and a guard written with any other arity is left standing.
fn guarded(ty: &syn::Type) -> &syn::Type {
    const GUARDS: [&str; 7] = ["Arc", "Rc", "Box", "Mutex", "RwLock", "RefCell", "Cell"];
    let syn::Type::Path(path) = ty else {
        return ty;
    };
    let Some(segment) = path.path.segments.last() else {
        return ty;
    };
    if !GUARDS.contains(&segment.ident.to_string().as_str()) {
        return ty;
    }
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return ty;
    };
    let written: Vec<&syn::Type> = arguments
        .args
        .iter()
        .filter_map(|argument| match argument {
            syn::GenericArgument::Type(inner) => Some(inner),
            _ => None
        })
        .collect();
    match written.as_slice() {
        [inner] => guarded(inner),
        _ => ty
    }
}

/// The type a collection field holds, read off the written type
/// arguments: the last one, so a map answers with its values and a
/// sequence with its elements. Guards around the collection are seen
/// through, so a map behind a lock holds what the map holds.
///
/// A type carrying no arguments holds nothing, and a type argument that
/// is not a plain path — a reference, a tuple, a slice — is not named
/// here, because what it holds has no single name to give.
fn held_element(ty: &syn::Type) -> Option<String> {
    let syn::Type::Path(path) = guarded(ty) else {
        return None;
    };
    let segment = path.path.segments.last()?;
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    let written: Vec<&syn::Type> = arguments
        .args
        .iter()
        .filter_map(|argument| match argument {
            syn::GenericArgument::Type(inner) => Some(inner),
            _ => None
        })
        .collect();
    let syn::Type::Path(inner) = written.last()? else {
        return None;
    };
    Some(inner.path.segments.last()?.ident.to_string())
}

/// The type a map is looked into with, where the field declares a map:
/// the first written type argument of a type carrying two. Guards around
/// the map are seen through, exactly as [`held_element`] sees them.
///
/// A type carrying one argument holds a sequence, looked into by
/// position, and answers nothing here.
fn looked_up_by(ty: &syn::Type) -> Option<String> {
    let syn::Type::Path(path) = guarded(ty) else {
        return None;
    };
    let segment = path.path.segments.last()?;
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    let written: Vec<&syn::Type> = arguments
        .args
        .iter()
        .filter_map(|argument| match argument {
            syn::GenericArgument::Type(inner) => Some(inner),
            _ => None
        })
        .collect();
    if written.len() < 2 {
        return None;
    }
    let syn::Type::Path(first) = written.first()? else {
        return None;
    };
    Some(first.path.segments.last()?.ident.to_string())
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
