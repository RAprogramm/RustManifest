// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Which functions take a path-extractor type apart in their signature,
//! and the one name each binds it to.
//!
//! A handler that receives a value the framework read out of the
//! request path says so in its own parameter list: the extractor type
//! is written there and the pattern takes it apart. The reading records
//! the function name and the bound name — a signature writing several
//! such inputs is skipped, because which of them names a record would
//! be a guess.

use crate::{semantic::surface::takes_from_path, source::Source};

/// Every function whose signature takes exactly one path-extractor type
/// apart into one name, free functions and impl methods alike, as
/// `(function, bound name)`.
#[must_use]
pub(super) fn path_takers(sources: &[Source]) -> Vec<(String, String)> {
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
                    if let Some(bound) = taken_apart(&function.sig) {
                        takers.push((function.sig.ident.to_string(), bound));
                    }
                }
                syn::Item::Impl(block) => {
                    for member in &block.items {
                        if let syn::ImplItem::Fn(method) = member
                            && let Some(bound) = taken_apart(&method.sig)
                        {
                            takers.push((method.sig.ident.to_string(), bound));
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

/// The one name a signature's single path-extractor pattern binds.
fn taken_apart(signature: &syn::Signature) -> Option<String> {
    let mut bound = signature.inputs.iter().filter_map(path_binding);
    let (first, rest) = (bound.next()?, bound.next());
    if rest.is_some() {
        return None;
    }
    Some(first)
}

/// The name one input binds through a path-extractor pattern, where the
/// input is written that way at all.
fn path_binding(input: &syn::FnArg) -> Option<String> {
    let syn::FnArg::Typed(argument) = input else {
        return None;
    };
    let syn::Type::Path(path) = argument.ty.as_ref() else {
        return None;
    };
    let named = path.path.segments.last()?.ident.to_string();
    if !takes_from_path(&named) {
        return None;
    }
    let syn::Pat::TupleStruct(pattern) = argument.pat.as_ref() else {
        return None;
    };
    let mut elems = pattern.elems.iter();
    let (first, rest) = (elems.next()?, elems.next());
    if rest.is_some() {
        return None;
    }
    let syn::Pat::Ident(ident) = first else {
        return None;
    };
    Some(ident.ident.to_string())
}
