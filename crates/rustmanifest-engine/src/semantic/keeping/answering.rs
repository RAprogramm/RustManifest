// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Which type each function answers with, read off its written return
//! type.
//!
//! The fallible half and the wrappers around the answer are seen
//! through: a handler returning a `Result` answers with its success
//! half, and a single-argument wrapper — a serializer, a pointer —
//! carries the type it wraps. What is left standing is the answer's own
//! name, and an opaque return — `impl Trait`, a bare primitive — simply
//! records what is written.

use crate::source::Source;

/// The type every function answers with, as `(function, type name)`,
/// free functions and impl methods alike. A function returning nothing
/// has no entry.
#[must_use]
pub(super) fn returners(sources: &[Source]) -> Vec<(String, String)> {
    let mut answers = Vec::new();
    for source in sources {
        let Some(file) = source.tree() else {
            continue;
        };
        let mut items = Vec::new();
        crate::ast::flattened(&file.items, &mut items);
        for item in items {
            match item {
                syn::Item::Fn(function) => {
                    if let Some(answer) = answered_with(&function.sig) {
                        answers.push((function.sig.ident.to_string(), answer));
                    }
                }
                syn::Item::Impl(block) => {
                    for member in &block.items {
                        if let syn::ImplItem::Fn(method) = member
                            && let Some(answer) = answered_with(&method.sig)
                        {
                            answers.push((method.sig.ident.to_string(), answer));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    answers.sort_unstable();
    answers.dedup();
    answers
}

/// The name of the type a signature answers with, wrappers and the
/// fallible half seen through.
fn answered_with(signature: &syn::Signature) -> Option<String> {
    let syn::ReturnType::Type(_, ty) = &signature.output else {
        return None;
    };
    unwrapped(ty)
}

/// The last path segment under the wrappers: `Result` answers with its
/// first written argument, and any type carrying exactly one written
/// argument carries what it wraps.
fn unwrapped(ty: &syn::Type) -> Option<String> {
    let syn::Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    let written: Vec<&syn::Type> = match &segment.arguments {
        syn::PathArguments::AngleBracketed(arguments) => arguments
            .args
            .iter()
            .filter_map(|argument| match argument {
                syn::GenericArgument::Type(inner) => Some(inner),
                _ => None
            })
            .collect(),
        _ => Vec::new()
    };
    if segment.ident == "Result" {
        return written.first().and_then(|inner| unwrapped(inner));
    }
    match written.as_slice() {
        [inner] => unwrapped(inner),
        [] => Some(segment.ident.to_string()),
        _ => None
    }
}
