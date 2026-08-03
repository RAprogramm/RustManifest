// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Return-type reading: what one signature's declared return says about
//! the value a call to it yields — the plain type, the success and error
//! types carried inside a `Result`/`Option`, and whether the call
//! returns to its caller at all.

/// Return type of one workspace function as the typing table sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Returned {
    /// Last segment of the written return type, `Self` resolved.
    pub(super) plain:   String,
    /// Success type inside a `Result`/`Option` return, `Self` resolved.
    pub(super) success: Option<String>,
    /// Error type inside a `Result` return, `Self` resolved.
    pub(super) error:   Option<String>,
    /// Whether the signature declares the never type, so a call to this
    /// function does not return to its caller.
    pub(super) never:   bool
}

/// Reads a signature's plain return type and, for `Result`/`Option`
/// returns, the success type carried in the first type argument.
pub(super) fn returned_of(signature: &syn::Signature, owner: Option<&str>) -> Option<Returned> {
    let syn::ReturnType::Type(_, ty) = &signature.output else {
        return None;
    };
    if matches!(ty.as_ref(), syn::Type::Never(_)) {
        return Some(Returned {
            plain:   "!".to_owned(),
            success: None,
            error:   None,
            never:   true
        });
    }
    let syn::Type::Path(path) = ty.as_ref() else {
        return None;
    };
    let segment = path.path.segments.last()?;
    let plain = resolve_self(segment.ident.to_string(), owner);
    let success = if segment.ident == "Result" || segment.ident == "Option" {
        type_argument(segment, 0).map(|name| resolve_self(name, owner))
    } else {
        None
    };
    let error = if segment.ident == "Result" {
        type_argument(segment, 1).map(|name| resolve_self(name, owner))
    } else {
        None
    };
    Some(Returned {
        plain,
        success,
        error,
        never: false
    })
}

/// Last segment of the `position`-th angle-bracketed type argument.
fn type_argument(segment: &syn::PathSegment, position: usize) -> Option<String> {
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    arguments
        .args
        .iter()
        .filter_map(|argument| {
            if let syn::GenericArgument::Type(syn::Type::Path(path)) = argument {
                path.path.segments.last().map(|s| s.ident.to_string())
            } else {
                None
            }
        })
        .nth(position)
}

/// Substitutes `Self` with the impl owner when one is known.
fn resolve_self(name: String, owner: Option<&str>) -> String {
    if name == "Self" {
        owner.map_or(name, str::to_owned)
    } else {
        name
    }
}
