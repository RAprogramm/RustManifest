// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The failure a return type declares: the error side of a `Result`.

use super::type_name::type_last_segment;

/// Extracts error type names from a `Result<_, E>` return type.
pub(in crate::semantic::behavior) fn error_types(signature: &syn::Signature) -> Vec<String> {
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
