// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The type vocabulary every other signature reading shares: how a type
//! written in a declaration renders as one name.

/// Renders the last path segment of a type, looking through references
/// and the std shared pointers (`Box`, `Rc`, `Arc`), whose deref makes
/// the wrapped type the receiver; a trait object renders as its first
/// trait bound, so calls through it resolve as dispatch over the
/// trait's implementors.
/// The name a written slice or array stands under. The language gives
/// the type no path of its own, and a call on one is a call on the
/// library's own sequence — never on anything a workspace declares — so
/// naming it keeps such a receiver out of the untyped row without
/// putting a workspace type in its place. No idiomatic Rust type is
/// spelled this way, so the name cannot collide with a written one.
pub(in crate::semantic::behavior) const SLICE: &str = "slice";

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
        syn::Type::Slice(_) | syn::Type::Array(_) => Some(SLICE.to_owned()),
        _ => None
    }
}
