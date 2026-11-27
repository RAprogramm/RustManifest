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
