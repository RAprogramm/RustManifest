// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Which types each function receives, read off its written parameter
//! types.
//!
//! The wrappers around a parameter are seen through exactly as they are
//! for an answer: a handler taking a deserializer wrapper receives the
//! type it wraps. What a name matched against this table means is the
//! matcher's question; this reading only states what is written.

use super::answering::unwrapped;
use crate::source::Source;

/// Every type a function's written parameters carry, as
/// `(function, type name)`, free functions and impl methods alike.
#[must_use]
pub(super) fn receivers(sources: &[Source]) -> Vec<(String, String)> {
    let mut received = Vec::new();
    for source in sources {
        let Some(file) = source.tree() else {
            continue;
        };
        let mut items = Vec::new();
        crate::ast::flattened(&file.items, &mut items);
        for item in items {
            match item {
                syn::Item::Fn(function) => {
                    collect(&function.sig, &mut received);
                }
                syn::Item::Impl(block) => {
                    for member in &block.items {
                        if let syn::ImplItem::Fn(method) = member {
                            collect(&method.sig, &mut received);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    received.sort_unstable();
    received.dedup();
    received
}

/// Records the unwrapped type of every written parameter of one
/// signature.
fn collect(signature: &syn::Signature, received: &mut Vec<(String, String)>) {
    for input in &signature.inputs {
        if let syn::FnArg::Typed(argument) = input
            && let Some(name) = unwrapped(&argument.ty)
        {
            received.push((signature.ident.to_string(), name));
        }
    }
}
