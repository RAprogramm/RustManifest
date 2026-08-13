// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The type name an expression settles to, for every expression the
//! recorded tables can answer for, and the type a `let` initializer
//! written as a literal or a constructor call carries on its face.

use super::FactCollector;

impl FactCollector {
    /// The type of an expression, as the compiler settled it.
    ///
    /// One answer and no second opinion: what a value is, is the
    /// compiler's to say, and a reading of how the source spelled it was a
    /// guess wearing the same shape. A position no resolver answered for
    /// is unknown here, and every reader above states that rather than
    /// filling it in.
    pub(super) fn expr_type(&self, expression: &syn::Expr) -> Option<String> {
        self.settled_expr_type(expression)
    }

    /// The type a resolver settled here, as the compiler renders it in
    /// full.
    pub(super) fn settled_expr_type_full(&self, expression: &syn::Expr) -> Option<String> {
        use syn::spanned::Spanned;

        let span = expression.span().byte_range();
        let start = u32::try_from(span.start).ok()?;
        let end = u32::try_from(span.end).ok()?;
        self.rendered.get(&(start, end)).cloned()
    }

    /// The type a resolver settled for the expression at this position.
    ///
    /// Asked first, and for every shape of expression, because it is the
    /// only answer here that was derived rather than inferred from
    /// spelling. What follows it is a reading of the written text, kept
    /// for sources no resolver ran over and for positions it left out.
    pub(super) fn settled_expr_type(&self, expression: &syn::Expr) -> Option<String> {
        use syn::spanned::Spanned;

        let span = expression.span().byte_range();
        let start = u32::try_from(span.start).ok()?;
        let end = u32::try_from(span.end).ok()?;
        self.typed.get(&(start, end)).cloned()
    }

    /// The type a resolver settled over this byte range.
    pub(super) fn settled_type_over(&self, span: &std::ops::Range<usize>) -> Option<String> {
        let start = u32::try_from(span.start).ok()?;
        let end = u32::try_from(span.end).ok()?;
        self.typed.get(&(start, end)).cloned()
    }
}

/// Infers the type name of a `let` initializer: a struct literal or a
/// `Type::new(..)` / `Type::default()` constructor call.
pub(super) fn initializer_type(expression: &syn::Expr) -> Option<String> {
    match expression {
        syn::Expr::Struct(literal) => literal
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        syn::Expr::Call(call) => {
            let syn::Expr::Path(path) = call.func.as_ref() else {
                return None;
            };
            let segments = &path.path.segments;
            if segments.len() < 2 {
                return None;
            }
            let method = segments.last()?.ident.to_string();
            if method != "new" && method != "default" {
                return None;
            }
            Some(segments[segments.len() - 2].ident.to_string())
        }
        syn::Expr::Reference(reference) => initializer_type(&reference.expr),
        _ => None
    }
}
