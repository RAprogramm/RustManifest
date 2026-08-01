// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Signature analysis: parameter ownership profiles, declared parameter
//! types, and error types extracted from a function's signature alone.
//! Everything here reads declarations, never bodies.

use std::collections::BTreeMap;

use super::{ParamFacts, ParamMode};

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

/// Renders the last path segment of a type, looking through references.
pub(super) fn type_last_segment(ty: &syn::Type) -> Option<String> {
    match ty {
        syn::Type::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        syn::Type::Reference(reference) => type_last_segment(&reference.elem),
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
