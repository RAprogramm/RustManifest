// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The declared parameter list: ownership profiles, types, and
//! collection element types, keyed by parameter name.



/// The one name a single-field tuple-struct pattern binds, where it
/// binds exactly one.
///
/// A parameter written as `Wrapper(name): Wrapper<T>` takes the wrapper
/// apart in the signature itself, so the name holds the wrapped value
/// from the first line of the body on.
pub(super) fn single_binding(pattern: &syn::PatTupleStruct) -> Option<String> {
    let mut bound = pattern.elems.iter();
    let (first, rest) = (bound.next()?, bound.next());
    if rest.is_some() {
        return None;
    }
    let syn::Pat::Ident(ident) = first else {
        return None;
    };
    Some(ident.ident.to_string())
}

