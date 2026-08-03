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
            syn::FnArg::Typed(argument) => {
                if let (syn::Pat::Ident(pattern), Some(type_name)) =
                    (argument.pat.as_ref(), type_last_segment(&argument.ty))
                {
                    let resolved = bounds.get(&type_name).cloned().unwrap_or(type_name);
                    types.insert(pattern.ident.to_string(), resolved);
                }
            }
        }
    }
    types
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
