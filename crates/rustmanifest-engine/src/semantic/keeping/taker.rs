// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Which functions take a path-extractor type apart in their signature.
//!
//! A handler that receives a value the framework read out of the
//! request path says so in its own parameter list: the extractor type
//! is written there and the pattern takes it apart. The reading records
//! the function names and nothing further — which parameter the value
//! names is settled where the route's own path is in hand.

use crate::{semantic::surface::takes_from_path, source::Source};

/// Names of every function whose signature takes a path-extractor type
/// apart, free functions and impl methods alike.
#[must_use]
pub(super) fn path_takers(sources: &[Source]) -> Vec<String> {
    let mut takers = Vec::new();
    for source in sources {
        let Some(file) = source.tree() else {
            continue;
        };
        let mut items = Vec::new();
        crate::ast::flattened(&file.items, &mut items);
        for item in items {
            match item {
                syn::Item::Fn(function) => {
                    if takes_path_apart(&function.sig) {
                        takers.push(function.sig.ident.to_string());
                    }
                }
                syn::Item::Impl(block) => {
                    for member in &block.items {
                        if let syn::ImplItem::Fn(method) = member
                            && takes_path_apart(&method.sig)
                        {
                            takers.push(method.sig.ident.to_string());
                        }
                    }
                }
                _ => {}
            }
        }
    }
    takers.sort_unstable();
    takers.dedup();
    takers
}

/// Whether a signature writes a path-extractor type and takes it apart.
fn takes_path_apart(signature: &syn::Signature) -> bool {
    signature.inputs.iter().any(|input| {
        let syn::FnArg::Typed(argument) = input else {
            return false;
        };
        let named = match argument.ty.as_ref() {
            syn::Type::Path(path) => path
                .path
                .segments
                .last()
                .map(|segment| segment.ident.to_string()),
            _ => None
        };
        matches!(argument.pat.as_ref(), syn::Pat::TupleStruct(_))
            && named.is_some_and(|name| takes_from_path(&name))
    })
}
