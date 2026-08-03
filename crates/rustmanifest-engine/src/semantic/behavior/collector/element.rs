// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Element typing: the element type an iterable hands out, the element
//! behind an optional accessor, and the binding that receives one.

use super::{FactCollector, catalog::ITERATOR_SOURCES, naming::binding_name};

impl FactCollector {
    /// Element type of an iterable expression: a binding or field of a
    /// known collection type, seen through references, parentheses, and
    /// the plain iterator constructors.
    pub(super) fn element_of(&self, expression: &syn::Expr) -> Option<String> {
        match expression {
            syn::Expr::Reference(reference) => self.element_of(&reference.expr),
            syn::Expr::Paren(inner) => self.element_of(&inner.expr),
            syn::Expr::Path(path) => path
                .path
                .get_ident()
                .and_then(|ident| self.elem_types.get(&ident.to_string()))
                .cloned(),
            syn::Expr::Field(field) => self
                .field_key(field)
                .and_then(|key| self.elem(&key))
                .cloned(),
            syn::Expr::MethodCall(call)
                if ITERATOR_SOURCES.contains(&call.method.to_string().as_str()) =>
            {
                self.element_of(&call.receiver)
            }
            _ => None
        }
    }

    /// Element type behind an `Option`-returning collection accessor —
    /// `get`, `first`, `last` — on a known element type.
    pub(super) fn optional_element(&self, expression: &syn::Expr) -> Option<String> {
        let syn::Expr::MethodCall(call) = expression else {
            return None;
        };
        if !matches!(call.method.to_string().as_str(), "get" | "first" | "last") {
            return None;
        }
        self.element_of(&call.receiver)
    }

    /// Binds one loop or closure pattern against the iterated element
    /// type, seeing through `enumerate`'s index tuple.
    pub(super) fn bind_element(&mut self, pattern: &syn::Pat, iterable: &syn::Expr) {
        if let syn::Expr::MethodCall(call) = iterable
            && call.method == "enumerate"
        {
            if let syn::Pat::Tuple(tuple) = pattern
                && tuple.elems.len() == 2
                && let (Some(name), Some(element)) = (
                    binding_name(&tuple.elems[1]),
                    self.element_of(&call.receiver)
                )
            {
                self.param_types.insert(name, element);
            }
            return;
        }
        if let (Some(name), Some(element)) = (binding_name(pattern), self.element_of(iterable)) {
            self.param_types.insert(name, element);
        }
    }
}
