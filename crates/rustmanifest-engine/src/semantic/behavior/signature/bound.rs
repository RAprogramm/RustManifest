// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Generic type parameters and the trait bound each one resolves to.

use std::collections::BTreeMap;

/// Maps each generic type parameter of the signature to its first trait
/// bound — the same rule trait objects already follow, so a call on a
/// generic parameter resolves as dispatch over the bound's
/// implementors. Inline bounds and `where` clauses both count; a
/// parameter with no trait bound stays unmapped.
pub(super) fn generic_bounds(signature: &syn::Signature) -> BTreeMap<String, String> {
    generics_bounds(&signature.generics)
}

/// Maps the type parameters of any generics list — a signature's or an
/// impl block's — to their first trait bound.
pub(in crate::semantic::behavior) fn generics_bounds(
    generics: &syn::Generics
) -> BTreeMap<String, String> {
    let mut bounds = BTreeMap::new();
    for parameter in &generics.params {
        if let syn::GenericParam::Type(type_param) = parameter
            && let Some(bound) = first_trait_bound(&type_param.bounds)
        {
            bounds.insert(type_param.ident.to_string(), bound);
        }
    }
    if let Some(clause) = &generics.where_clause {
        for predicate in &clause.predicates {
            if let syn::WherePredicate::Type(typed) = predicate
                && let syn::Type::Path(path) = &typed.bounded_ty
                && let Some(name) = path.path.get_ident().map(ToString::to_string)
                && let Some(bound) = first_trait_bound(&typed.bounds)
            {
                bounds.entry(name).or_insert(bound);
            }
        }
    }
    bounds
}

/// Last segment of the first trait bound in a bound list, `?Sized` and
/// lifetimes skipped.
fn first_trait_bound(
    bounds: &syn::punctuated::Punctuated<syn::TypeParamBound, syn::Token![+]>
) -> Option<String> {
    bounds.iter().find_map(|bound| {
        if let syn::TypeParamBound::Trait(trait_bound) = bound
            && trait_bound.maybe.is_none()
        {
            trait_bound
                .path
                .segments
                .last()
                .map(|segment| segment.ident.to_string())
        } else {
            None
        }
    })
}
