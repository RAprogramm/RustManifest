// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Whether a piece of syntax names a router value: the type of a
//! parameter or a local, the name a pattern binds, and the associated
//! constructor that roots a router chain.

use syn::Expr;

use super::framework::router_type;

/// Parameter names whose declared type is one of the router types.
pub(super) fn router_inputs(signature: &syn::Signature) -> Vec<String> {
    let mut out = Vec::new();
    for input in &signature.inputs {
        let syn::FnArg::Typed(typed) = input else {
            continue;
        };
        let (Some(name), Some(kind)) = (binding_name(&typed.pat), type_name(&typed.ty)) else {
            continue;
        };
        if router_type(&kind) {
            out.push(name);
        }
    }
    out
}

/// Last path segment of a type, when the type is a plain path.
pub(super) fn type_name(kind: &syn::Type) -> Option<String> {
    match kind {
        syn::Type::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        syn::Type::Reference(reference) => type_name(&reference.elem),
        syn::Type::Paren(inner) => type_name(&inner.elem),
        syn::Type::Group(inner) => type_name(&inner.elem),
        _ => None
    }
}

/// Name bound by a pattern, when the pattern binds exactly one.
pub(super) fn binding_name(pattern: &syn::Pat) -> Option<String> {
    match pattern {
        syn::Pat::Ident(ident) => Some(ident.ident.to_string()),
        syn::Pat::Type(typed) => binding_name(&typed.pat),
        _ => None
    }
}

/// Whether a pattern's type annotation names a router type.
pub(super) fn router_typed(pattern: &syn::Pat) -> bool {
    let syn::Pat::Type(typed) = pattern else {
        return false;
    };
    type_name(&typed.ty).is_some_and(|name| router_type(&name))
}

/// Whether a called path is a router type's associated constructor, such
/// as `Router::new` or `axum::Router::<S>::new`.
pub(super) fn router_constructor(expression: &Expr) -> bool {
    let Expr::Path(path) = expression else {
        return false;
    };
    path.path
        .segments
        .iter()
        .rev()
        .nth(1)
        .is_some_and(|segment| router_type(&segment.ident.to_string()))
}
