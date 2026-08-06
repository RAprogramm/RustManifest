// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The element type a declared collection carries: slices, arrays, and
//! the std sequence containers, read from the declaration alone — and,
//! for a map, the pair its iteration hands out, position by position.

use super::type_name::type_last_segment;

/// The key and value types a declared map carries, in that order.
///
/// Iterating a map hands out a pair, and which of the two stands at
/// which position is fixed by the library rather than by the reading. A
/// container that is not one of the two standard maps is not answered
/// for at all.
pub(in crate::semantic::behavior) fn pair_types(ty: &syn::Type) -> Option<(String, String)> {
    match ty {
        syn::Type::Reference(reference) => pair_types(&reference.elem),
        syn::Type::Path(path) => {
            let segment = path.path.segments.last()?;
            if !matches!(segment.ident.to_string().as_str(), "BTreeMap" | "HashMap") {
                return None;
            }
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
            let [key, value] = written[..] else {
                return None;
            };
            Some((concrete_element(key)?, concrete_element(value)?))
        }
        _ => None
    }
}

/// Element type of a collection type.
pub(in crate::semantic::behavior) fn element_type(ty: &syn::Type) -> Option<String> {
    match ty {
        syn::Type::Reference(reference) => element_type(&reference.elem),
        syn::Type::Slice(slice) => concrete_element(&slice.elem),
        syn::Type::Array(array) => concrete_element(&array.elem),
        syn::Type::Path(path) => {
            let segment = path.path.segments.last()?;
            let name = segment.ident.to_string();
            if !matches!(name.as_str(), "Vec" | "VecDeque" | "BTreeSet" | "HashSet") {
                return None;
            }
            let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
                return None;
            };
            arguments.args.iter().find_map(|argument| {
                if let syn::GenericArgument::Type(inner) = argument {
                    concrete_element(inner)
                } else {
                    None
                }
            })
        }
        _ => None
    }
}

/// Last segment of an element type, seeing through the std smart
/// pointers; a boxed trait object renders as its trait, resolved later
/// as dispatch over the trait's implementors.
fn concrete_element(ty: &syn::Type) -> Option<String> {
    if let syn::Type::Path(path) = ty
        && let Some(segment) = path.path.segments.last()
        && matches!(segment.ident.to_string().as_str(), "Box" | "Rc" | "Arc")
    {
        let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
            return None;
        };
        return arguments.args.iter().find_map(|argument| {
            if let syn::GenericArgument::Type(inner) = argument {
                concrete_element(inner)
            } else {
                None
            }
        });
    }
    type_last_segment(ty)
}
