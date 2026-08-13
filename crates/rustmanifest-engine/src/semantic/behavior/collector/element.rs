// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Element typing: the element type an iterable hands out, the element
//! behind an optional accessor, and the binding that receives one.

use super::{
    FactCollector,
    catalog::{ELEMENT_PRESERVING, ITERATOR_SOURCES},
    naming::binding_name
};

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
                .cloned()
                .or_else(|| self.packed_field_element(field)),
            syn::Expr::MethodCall(call) if follows(&call.method.to_string()) => {
                self.element_of(&call.receiver)
            }
            syn::Expr::MethodCall(call) => self.packed_element(call),
            _ => None
        }
    }

    /// What one step of a sequence field a pack knows carries.
    fn packed_field_element(&self, field: &syn::ExprField) -> Option<String> {
        let owner = self.expr_type(&field.base)?;
        let member = super::naming::member_name(&field.member);
        crate::semantic::knowledge::field_element(&owner, &member).map(ToOwned::to_owned)
    }

    /// What one step of a sequence a pack knows carries: the receiver's
    /// own type answers which sequence is being read.
    fn packed_element(&self, call: &syn::ExprMethodCall) -> Option<String> {
        let owner = self.expr_type(&call.receiver)?;
        crate::semantic::knowledge::iterated_element(&owner, &call.method.to_string())
            .map(ToOwned::to_owned)
    }

    /// Key and value types an iterated map hands out as a pair, seen
    /// through the same chain the element reading follows.
    pub(super) fn pair_of(&self, expression: &syn::Expr) -> Option<(String, String)> {
        match expression {
            syn::Expr::Reference(reference) => self.pair_of(&reference.expr),
            syn::Expr::Paren(inner) => self.pair_of(&inner.expr),
            syn::Expr::Path(path) => path
                .path
                .get_ident()
                .and_then(|ident| self.local_pairs.get(&ident.to_string()))
                .cloned(),
            syn::Expr::Field(field) => self
                .field_key(field)
                .and_then(|key| self.field_pairs.resolve(&key, self.src, &self.crates))
                .cloned(),
            syn::Expr::MethodCall(call) if follows(&call.method.to_string()) => {
                self.pair_of(&call.receiver)
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
        if !matches!(
            call.method.to_string().as_str(),
            "get" | "get_mut" | "first" | "first_mut" | "last" | "last_mut"
        ) {
            return None;
        }
        self.element_of(&call.receiver)
    }

    /// Lets the input of every closure written beside an adapter stand
    /// for an element of what the adapter runs over.
    ///
    /// The same fact a loop binding carries: the closure is handed one
    /// element at a time, and which value that is comes from the
    /// receiver. Recorded as the same selection, so a reading does not
    /// depend on whether the source wrote a loop or an adapter.
    pub(super) fn bind_adapter_inputs(
        &mut self,
        call: &syn::ExprMethodCall,
        span: &std::ops::Range<usize>
    ) {
        for argument in &call.args {
            let syn::Expr::Closure(closure) = argument else {
                continue;
            };
            let [only] = closure.inputs.iter().collect::<Vec<_>>()[..] else {
                continue;
            };
            self.record_iterated_element(only, &call.receiver, span);
        }
    }

    /// Binds one loop or closure pattern against the iterated element
    /// type, seeing through `enumerate`'s index tuple and through the
    /// pair a map hands out.
    pub(super) fn bind_element(&mut self, pattern: &syn::Pat, iterable: &syn::Expr) {
        if let syn::Pat::Tuple(tuple) = pattern
            && tuple.elems.len() == 2
            && let Some((key, value)) = self.pair_of(iterable)
        {
            if let Some(name) = binding_name(&tuple.elems[0]) {
                self.param_types.insert(name, key);
            }
            if let Some(name) = binding_name(&tuple.elems[1]) {
                self.param_types.insert(name, value);
            }
            return;
        }
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

/// Whether a method hands on what its receiver holds: an iteration
/// constructor, or an adapter that keeps the element as it was.
fn follows(method: &str) -> bool {
    ITERATOR_SOURCES.contains(&method) || ELEMENT_PRESERVING.contains(&method)
}
