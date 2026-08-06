// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The type name an expression settles to, for every expression the
//! recorded tables can answer for, and the type a `let` initializer
//! written as a literal or a constructor call carries on its face.

use super::FactCollector;

impl FactCollector {
    /// Types any expression the tables can settle: bindings and fields
    /// by their recorded types, indexed collections by element, calls
    /// through [`Self::called_return`].
    ///
    /// A borrow, a parenthesis, a group and a dereference are seen
    /// through: they change how a value is held, never which value it is,
    /// and a method written on a borrow is the method of the type behind
    /// it. Reading only the bare form left every borrowed receiver
    /// untyped, which is most of them.
    pub(super) fn expr_type(&self, expression: &syn::Expr) -> Option<String> {
        match expression {
            syn::Expr::Reference(borrowed) => self.expr_type(&borrowed.expr),
            syn::Expr::Paren(inner) => self.expr_type(&inner.expr),
            syn::Expr::Group(inner) => self.expr_type(&inner.expr),
            syn::Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Deref(_)) => {
                self.expr_type(&unary.expr)
            }
            syn::Expr::Path(path) => path
                .path
                .get_ident()
                .and_then(|ident| self.param_types.get(&ident.to_string()))
                .map(|name| self.bounds.get(name).unwrap_or(name).clone()),
            syn::Expr::Field(field) => {
                if let Some(settled) = self.settled_field_type(field) {
                    return Some(settled);
                }
                let owner = self.expr_type(&field.base)?;
                self.field(&format!(
                    "{owner}.{}",
                    super::naming::member_name(&field.member)
                ))
                .map(|inner| self.bounds.get(inner).unwrap_or(inner).clone())
            }
            syn::Expr::Index(indexed) => self.element_of(&indexed.expr),
            other => self.called_return(other)
        }
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
