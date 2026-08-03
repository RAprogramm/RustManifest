// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The transparent wrappings this tier sees through on the way to the
//! expression that actually decides an answer: parentheses and invisible
//! groupings, borrows and dereferences, and the pinning constructor.

/// The expression inside the parentheses and invisible groupings around
/// it — a closure invoked where it is written has to be parenthesized,
/// so a recognizer that skips them never sees one.
pub(super) fn unwrapped(expression: &syn::Expr) -> &syn::Expr {
    match expression {
        syn::Expr::Paren(inner) => unwrapped(&inner.expr),
        syn::Expr::Group(inner) => unwrapped(&inner.expr),
        _ => expression
    }
}

/// Strips leading references and dereferences off an expression.
pub(super) fn strip_ref_deref(expression: &syn::Expr) -> &syn::Expr {
    match expression {
        syn::Expr::Reference(reference) => strip_ref_deref(&reference.expr),
        syn::Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Deref(_)) => {
            strip_ref_deref(&unary.expr)
        }
        _ => expression
    }
}

/// Inner expression of a `Pin::new(&mut *value)` call: `Pin` is a
/// transparent pointer for receiver typing, so the pinned value's type
/// is the wrapped expression's type, references and derefs stripped.
pub(super) fn pinned_inner(call: &syn::ExprCall) -> Option<&syn::Expr> {
    let syn::Expr::Path(path) = call.func.as_ref() else {
        return None;
    };
    let segments = &path.path.segments;
    if segments.len() < 2
        || segments[segments.len() - 1].ident != "new"
        || segments[segments.len() - 2].ident != "Pin"
    {
        return None;
    }
    call.args.first().map(strip_ref_deref)
}
