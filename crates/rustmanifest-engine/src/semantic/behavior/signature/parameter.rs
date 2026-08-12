// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The declared parameter list: ownership profiles, types, and
//! collection element types, keyed by parameter name.

use std::collections::BTreeMap;

use super::{
    bound::generic_bounds,
    element::element_type,
    type_name::{type_last_segment, written_type_segment}
};
use crate::semantic::behavior::types::{ParamFacts, ParamMode};

/// Builds the ownership profile of every parameter, `self` included.
pub(in crate::semantic::behavior) fn param_facts(
    signature: &syn::Signature,
    impl_type: Option<&str>
) -> Vec<ParamFacts> {
    let mut params = Vec::new();
    for input in &signature.inputs {
        match input {
            syn::FnArg::Receiver(receiver) => {
                let Some(impl_type) = impl_type else {
                    continue;
                };
                let mode = match receiver.kind {
                    syn::ReceiverKind::Reference(_, _, Some(_)) => ParamMode::BorrowedMut,
                    syn::ReceiverKind::Reference(..) => ParamMode::Borrowed,
                    _ => ParamMode::Owned
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
                let Some(type_name) = written_type_segment(&argument.ty) else {
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
pub(in crate::semantic::behavior) fn parameter_types(
    signature: &syn::Signature,
    impl_type: Option<&str>
) -> BTreeMap<String, String> {
    let bounds = generic_bounds(signature);
    let mut types = BTreeMap::new();
    for input in &signature.inputs {
        match input {
            syn::FnArg::Receiver(_) => {
                if let Some(impl_type) = impl_type {
                    types.insert("self".to_owned(), impl_type.to_owned());
                }
            }
            syn::FnArg::Typed(argument) => match argument.pat.as_ref() {
                syn::Pat::Ident(pattern) => {
                    if let Some(type_name) = type_last_segment(&argument.ty) {
                        let resolved = bounds.get(&type_name).cloned().unwrap_or(type_name);
                        types.insert(pattern.ident.to_string(), resolved);
                    }
                }
                syn::Pat::TupleStruct(pattern) => {
                    if let (Some(name), Some(type_name)) =
                        (single_binding(pattern), unwrapped_argument(&argument.ty))
                    {
                        types.insert(name, type_name);
                    }
                }
                _ => {}
            }
        }
    }
    types
}

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

/// The type the one written argument of a wrapper names, where the
/// wrapper carries exactly one.
///
/// Read for a parameter whose pattern already proves the unwrapping:
/// the single field of such a wrapper is taken to be its single type
/// argument, which holds for the extractor shape the pattern is written
/// for, and is wrong where a one-argument type declares its field as
/// something else — the same class of approximation as the wrapped
/// closure binding, and bounded the same way by the written pattern.
fn unwrapped_argument(ty: &syn::Type) -> Option<String> {
    let syn::Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    let mut written = arguments.args.iter().filter_map(|argument| {
        if let syn::GenericArgument::Type(inner) = argument {
            Some(inner)
        } else {
            None
        }
    });
    let (first, rest) = (written.next()?, written.next());
    if rest.is_some() {
        return None;
    }
    type_last_segment(first)
}

/// Maps parameter names to the key and value types of their declared
/// map type, so a tuple destructured from iterating one is typed on
/// both sides.
pub(in crate::semantic::behavior) fn parameter_pairs(
    signature: &syn::Signature
) -> BTreeMap<String, (String, String)> {
    let mut pairs = BTreeMap::new();
    for input in &signature.inputs {
        if let syn::FnArg::Typed(argument) = input
            && let (syn::Pat::Ident(pattern), Some(pair)) = (
                argument.pat.as_ref(),
                super::element::pair_types(&argument.ty)
            )
        {
            pairs.insert(pattern.ident.to_string(), pair);
        }
    }
    pairs
}

/// Maps parameter names to the element type of their declared
/// collection type, when one is evident.
pub(in crate::semantic::behavior) fn parameter_elements(
    signature: &syn::Signature
) -> BTreeMap<String, String> {
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
