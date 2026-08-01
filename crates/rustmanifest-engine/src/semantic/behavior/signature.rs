// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Signature analysis: parameter ownership profiles, declared parameter
//! types, and error types extracted from a function's signature alone.
//! Everything here reads declarations, never bodies.

use std::collections::BTreeMap;

use super::types::{ParamFacts, ParamMode};

/// Builds the ownership profile of every parameter, `self` included.
pub(super) fn param_facts(signature: &syn::Signature, impl_type: Option<&str>) -> Vec<ParamFacts> {
    let mut params = Vec::new();
    for input in &signature.inputs {
        match input {
            syn::FnArg::Receiver(receiver) => {
                let Some(impl_type) = impl_type else {
                    continue;
                };
                let mode = if receiver.reference.is_none() {
                    ParamMode::Owned
                } else if receiver.mutability.is_some() {
                    ParamMode::BorrowedMut
                } else {
                    ParamMode::Borrowed
                };
                params.push(ParamFacts {
                    name: "self".to_owned(),
                    type_name: impl_type.to_owned(),
                    mode,
                    uses: 0,
                    field_reads: 0
                });
            }
            syn::FnArg::Typed(argument) => {
                let syn::Pat::Ident(pattern) = argument.pat.as_ref() else {
                    continue;
                };
                let Some(type_name) = type_last_segment(&argument.ty) else {
                    continue;
                };
                let mode = match argument.ty.as_ref() {
                    syn::Type::Reference(reference) if reference.mutability.is_some() => {
                        ParamMode::BorrowedMut
                    }
                    syn::Type::Reference(_) => ParamMode::Borrowed,
                    _ => ParamMode::Owned
                };
                params.push(ParamFacts {
                    name: pattern.ident.to_string(),
                    type_name,
                    mode,
                    uses: 0,
                    field_reads: 0
                });
            }
        }
    }
    params
}

/// Maps parameter names to the last segment of their declared type;
/// `self` maps to the impl type.
pub(super) fn parameter_types(
    signature: &syn::Signature,
    impl_type: Option<&str>
) -> BTreeMap<String, String> {
    let mut types = BTreeMap::new();
    for input in &signature.inputs {
        match input {
            syn::FnArg::Receiver(_) => {
                if let Some(impl_type) = impl_type {
                    types.insert("self".to_owned(), impl_type.to_owned());
                }
            }
            syn::FnArg::Typed(argument) => {
                if let (syn::Pat::Ident(pattern), Some(type_name)) =
                    (argument.pat.as_ref(), type_last_segment(&argument.ty))
                {
                    types.insert(pattern.ident.to_string(), type_name);
                }
            }
        }
    }
    types
}

/// Renders the element type of a collection: slices, arrays, and the
/// std sequence containers carrying one type argument, looking through
/// references.
pub(super) fn element_type(ty: &syn::Type) -> Option<String> {
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

/// Maps parameter names to the element type of their declared
/// collection type, when one is evident.
pub(super) fn parameter_elements(signature: &syn::Signature) -> BTreeMap<String, String> {
    let mut elements = BTreeMap::new();
    for input in &signature.inputs {
        if let syn::FnArg::Typed(argument) = input
            && let (syn::Pat::Ident(pattern), Some(element)) =
                (argument.pat.as_ref(), element_type(&argument.ty))
        {
            elements.insert(pattern.ident.to_string(), element);
        }
    }
    elements
}

/// Renders the last path segment of a type, looking through references;
/// a trait object renders as its first trait bound, so calls through it
/// resolve as dispatch over the trait's implementors.
pub(super) fn type_last_segment(ty: &syn::Type) -> Option<String> {
    match ty {
        syn::Type::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
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

/// Extracts error type names from a `Result<_, E>` return type.
pub(super) fn error_types(signature: &syn::Signature) -> Vec<String> {
    let syn::ReturnType::Type(_, ty) = &signature.output else {
        return Vec::new();
    };
    let syn::Type::Path(path) = ty.as_ref() else {
        return Vec::new();
    };
    let Some(segment) = path.path.segments.last() else {
        return Vec::new();
    };
    if segment.ident != "Result" {
        return Vec::new();
    }
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return Vec::new();
    };
    arguments
        .args
        .iter()
        .skip(1)
        .filter_map(|argument| {
            let syn::GenericArgument::Type(ty) = argument else {
                return None;
            };
            type_last_segment(ty)
        })
        .collect()
}
