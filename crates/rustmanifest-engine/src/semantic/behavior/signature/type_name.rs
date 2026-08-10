// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The type vocabulary every other signature reading shares: how a type
//! written in a declaration renders as one name.

/// Renders the last path segment of the type as written, references
/// stripped but smart pointers kept — the shape ownership advice
/// reasons about: an owned `Arc` parameter is idiomatic sharing, not a
/// borrow candidate. A slice or array renders as its bracketed element
/// (`&[u8]` → `[u8]`): the borrow of it is provable from the written
/// `&` alone, and dropping the parameter for an unreadable name hid
/// that provable fact.
pub(super) fn written_type_segment(ty: &syn::Type) -> Option<String> {
    match ty {
        syn::Type::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        syn::Type::Reference(reference) => written_type_segment(&reference.elem),
        syn::Type::Slice(slice) => {
            written_type_segment(&slice.elem).map(|inner| format!("[{inner}]"))
        }
        syn::Type::Array(array) => {
            written_type_segment(&array.elem).map(|inner| format!("[{inner}]"))
        }
        other => type_last_segment(other)
    }
}

/// Renders the last path segment of a type, looking through references
/// and the std shared pointers (`Box`, `Rc`, `Arc`), whose deref makes
/// the wrapped type the receiver; a trait object renders as its first
/// trait bound, so calls through it resolve as dispatch over the
/// trait's implementors.
pub(in crate::semantic::behavior) fn type_last_segment(ty: &syn::Type) -> Option<String> {
    match ty {
        syn::Type::Path(path) => {
            let segment = path.path.segments.last()?;
            if matches!(segment.ident.to_string().as_str(), "Box" | "Rc" | "Arc")
                && let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments
            {
                let inner = arguments.args.iter().find_map(|argument| {
                    if let syn::GenericArgument::Type(inner) = argument {
                        type_last_segment(inner)
                    } else {
                        None
                    }
                });
                if inner.is_some() {
                    return inner;
                }
            }
            Some(segment.ident.to_string())
        }
        syn::Type::Reference(reference) => type_last_segment(&reference.elem),
        syn::Type::TraitObject(object) => object.bounds.iter().find_map(|bound| {
            if let syn::TypeParamBound::Trait(trait_bound) = bound {
                trait_bound
                    .path
                    .segments
                    .last()
                    .map(|segment| segment.ident.to_string())
            } else {
                None
            }
        }),
        _ => None
    }
}
