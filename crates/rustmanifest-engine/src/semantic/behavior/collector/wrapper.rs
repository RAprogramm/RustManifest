// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Wrapper typing: the inner type a single-generic wrapper carries, and
//! the closure parameter a combinator hands that inner value to.

use super::{
    FactCollector,
    naming::binding_name,
    unwrapping::strip_ref_deref
};

impl FactCollector {
    /// Wrapped inner type of a wrapper expression: a field with a
    /// recorded single-generic inner, a binding that inherited one, or
    /// a `clone` of either — references and derefs stripped.
    pub(super) fn wrapped_of(&self, expression: &syn::Expr) -> Option<String> {
        match strip_ref_deref(expression) {
            syn::Expr::Path(path) => path
                .path
                .get_ident()
                .and_then(|ident| self.local_wraps.get(&ident.to_string()))
                .cloned(),
            syn::Expr::Field(field) => {
                self.settled_expr_type(&syn::Expr::Field(field.clone()))
            }
            syn::Expr::MethodCall(call) if call.method == "clone" => {
                self.wrapped_of(&call.receiver)
            }
            _ => None
        }
    }

    /// The value a guard-opening chain leaves in hand: `lock`, `read`,
    /// `write`, `borrow` and `borrow_mut` open a wrapper into a guard
    /// that stands for the wrapped value, and `unwrap` or `expect` only
    /// strips the fallible half around that guard. Any other method ends
    /// the chain, and a receiver that is no recorded wrapper answers
    /// nothing — `read` on a socket stays a read on a socket.
    pub(super) fn opened_of(&self, expression: &syn::Expr) -> Option<String> {
        let syn::Expr::MethodCall(call) = strip_ref_deref(expression) else {
            return None;
        };
        match call.method.to_string().as_str() {
            "unwrap" | "expect" => self.opened_of(&call.receiver),
            "lock" | "read" | "write" | "borrow" | "borrow_mut" => self.wrapped_of(&call.receiver),
            _ => None
        }
    }

    /// Types the parameter of a single-parameter closure argument with
    /// the receiver field's wrapped inner type — the `do_io(|inner|
    /// inner.read(buf))` shape, where the wrapper hands its wrapped
    /// value to the closure. The same class of approximation as the
    /// iterator-adapter element binding, and wrong the same way a
    /// combinator passing something other than the wrapped value would
    /// make it; single-generic wrappers keep that surface small.
    pub(super) fn bind_wrapped_closure(&mut self, call: &syn::ExprMethodCall) {
        let Some(inner) = self.wrapped_of(&call.receiver) else {
            return;
        };
        for argument in &call.args {
            if let syn::Expr::Closure(closure) = argument
                && closure.inputs.len() == 1
                && let Some(pattern) = closure.inputs.first()
                && let Some(name) = binding_name(pattern)
            {
                self.param_types.insert(name, inner.clone());
            }
        }
    }
}
