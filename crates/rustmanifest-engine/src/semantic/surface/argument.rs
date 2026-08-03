// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Reading the arguments of a registration or composition call: the path
//! literal, the verbs and handlers of a method-router, and the segments
//! naming the provider a composition folds in.

use syn::Expr;

use super::framework::FrameworkModel;

/// Reads a string literal argument.
pub(super) fn literal(expression: &Expr) -> Option<String> {
    let Expr::Lit(literal) = expression else {
        return None;
    };
    match &literal.lit {
        syn::Lit::Str(text) => Some(text.value()),
        _ => None
    }
}

/// Reads the `(method, handler)` pairs of a method-router argument,
/// following `get(h).post(other)` chains.
pub(super) fn method_router(model: &FrameworkModel, expression: &Expr) -> Vec<(String, String)> {
    match expression {
        Expr::Call(call) => {
            let Some(verb) =
                call_name(&call.func).filter(|name| model.methods.contains(&name.as_str()))
            else {
                return Vec::new();
            };
            call.args
                .first()
                .and_then(handler_name)
                .map(|handler| vec![(verb.to_uppercase(), handler)])
                .unwrap_or_default()
        }
        Expr::MethodCall(call) => {
            let verb = call.method.to_string();
            let mut pairs = method_router(model, &call.receiver);
            if model.methods.contains(&verb.as_str())
                && let Some(handler) = call.args.first().and_then(handler_name)
            {
                pairs.push((verb.to_uppercase(), handler));
            }
            pairs
        }
        _ => Vec::new()
    }
}

/// Last path segment of a called function expression.
fn call_name(expression: &Expr) -> Option<String> {
    let Expr::Path(path) = expression else {
        return None;
    };
    path.path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
}

/// Handler name of a method-router argument: a plain path, or the base
/// of a chain such as `handler.layer(..)`.
fn handler_name(expression: &Expr) -> Option<String> {
    match expression {
        Expr::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        Expr::MethodCall(call) => handler_name(&call.receiver),
        _ => None
    }
}

/// Path segments naming the provider a composition argument calls:
/// `routes()` or `module::routes()`, including a chain built on top of
/// such a call. Leading `crate`/`self`/`super` qualifiers are dropped so
/// the remainder can be matched against provider keys.
pub(super) fn provider_reference(expression: &Expr) -> Option<Vec<String>> {
    match expression {
        Expr::Call(call) => path_segments(&call.func),
        Expr::MethodCall(call) => provider_reference(&call.receiver),
        Expr::Paren(inner) => provider_reference(&inner.expr),
        Expr::Group(inner) => provider_reference(&inner.expr),
        Expr::Await(inner) => provider_reference(&inner.base),
        Expr::Try(inner) => provider_reference(&inner.expr),
        _ => None
    }
}

/// Segments of a path expression, without its leading path qualifiers.
fn path_segments(expression: &Expr) -> Option<Vec<String>> {
    let Expr::Path(path) = expression else {
        return None;
    };
    let segments: Vec<String> = path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .skip_while(|segment| matches!(segment.as_str(), "crate" | "self" | "super" | "Self"))
        .collect();
    if segments.is_empty() {
        return None;
    }
    Some(segments)
}
