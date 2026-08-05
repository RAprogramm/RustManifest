// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Facts read off an invocation: the callee a path call names, everything
//! a method call carries — the writes it performs, the clones and the
//! output it produces, the bindings it types and the effects it reaches —
//! and the poll an awaited future stands for.

use super::{
    FactCollector,
    catalog::{BYTE_WRITE_METHOD, ITERATOR_ADAPTERS, TEXT_WRITE_METHOD, VALUE_WRAPPERS}
};
use crate::semantic::{expr::MUTATING_METHODS, knowledge, std_knowledge::Carried};

impl FactCollector {
    /// Records the callee of a call written as a path, qualified by the
    /// owner segment where the path carries one.
    pub(super) fn record_called_path(&mut self, call: &syn::ExprCall) {
        use syn::spanned::Spanned;

        let span = call.span().byte_range();
        let mut reserved = None;
        self.record_source_call(call);
        if let syn::Expr::Path(path) = call.func.as_ref() {
            let segments = &path.path.segments;
            if segments.len() == 1 {
                reserved =
                    Some(self.reserve_written_call(None, &segments[0].ident.to_string(), &span));
            } else if segments.len() >= 2 {
                let owner = segments[segments.len() - 2].ident.to_string();
                let callee = segments[segments.len() - 1].ident.to_string();
                reserved = Some(self.reserve_written_call(Some(&owner), &callee, &span));
            }
        }
        syn::visit::visit_expr_call(self, call);
        if let Some(index) = reserved {
            let arguments: Vec<&syn::Expr> = call.args.iter().collect();
            self.settle_arguments(index, &arguments);
        }
        self.carry_wrapper(call, &span);
    }

    /// Hands on the origin of the one value a library wrapper is built
    /// around.
    ///
    /// The value such a construction stands for is the value written
    /// inside it; naming the construction instead would put a step
    /// between a value and itself, and a term full of those steps says
    /// less with every one of them.
    fn carry_wrapper(&mut self, call: &syn::ExprCall, span: &std::ops::Range<usize>) {
        let syn::Expr::Path(path) = call.func.as_ref() else {
            return;
        };
        let Some(named) = path.path.segments.last() else {
            return;
        };
        if !VALUE_WRAPPERS.contains(&named.ident.to_string().as_str()) {
            return;
        }
        let [only] = call.args.iter().collect::<Vec<_>>()[..] else {
            return;
        };
        let carried = self.origin_of(only);
        self.produced.insert(
            (
                u32::try_from(span.start).unwrap_or(u32::MAX),
                u32::try_from(span.end).unwrap_or(u32::MAX)
            ),
            carried
        );
    }

    /// Records everything one method call carries.
    pub(super) fn collect_method_call(&mut self, call: &syn::ExprMethodCall) {
        let method = call.method.to_string();
        if MUTATING_METHODS.contains(&method.as_str())
            && let syn::Expr::Field(field) = call.receiver.as_ref()
            && let Some(key) = self.field_key(field)
        {
            self.record_field_write(field, key);
        }
        if method == "clone" || method == "to_owned" || method == "to_string" {
            self.facts.clones += 1;
        }
        if method == BYTE_WRITE_METHOD || method == TEXT_WRITE_METHOD {
            self.record_byte_output(call);
        }
        if ITERATOR_ADAPTERS.contains(&method.as_str()) {
            for argument in &call.args {
                if let syn::Expr::Closure(closure) = argument
                    && closure.inputs.len() == 1
                    && let Some(pattern) = closure.inputs.first()
                {
                    self.bind_element(pattern, &call.receiver);
                }
            }
        } else {
            self.bind_wrapped_closure(call);
        }
        if method == knowledge::SERVICE_CALL
            && let Some(inner) = self.wrapped_of(&call.receiver)
        {
            self.record_call(Some(&inner), &method);
        }
        let span = {
            use syn::spanned::Spanned;
            call.span().byte_range()
        };
        let reserved = match self.expr_type(&call.receiver) {
            Some(owner) => {
                if let Some(effect) = knowledge::method_effect(&owner, &method)
                    .or_else(|| crate::semantic::std_knowledge::method_effect(&owner, &method))
                {
                    self.facts.effects.push(effect);
                }
                self.reserve_written_call(Some(&owner), &method, &span)
            }
            None => self.reserve_written_call(Some("?"), &method, &span)
        };
        syn::visit::visit_expr_method_call(self, call);
        let mut written: Vec<&syn::Expr> = vec![call.receiver.as_ref()];
        written.extend(call.args.iter());
        self.settle_arguments(reserved, &written);
        if self.record_read_method(call).is_none() && !self.follow_intake(call) {
            self.record_method_operation(call);
        }
        self.carry_outcome(call, &span);
    }

    /// Hands on the origin a library call carries, where a row says which
    /// of its inputs it carries.
    ///
    /// The value such a call answers with is a value it was given, so
    /// naming the call would put a step between a value and itself. A
    /// method whose answer is derived rather than carried has no row and
    /// is not seen through.
    fn carry_outcome(&mut self, call: &syn::ExprMethodCall, span: &std::ops::Range<usize>) {
        let method = call.method.to_string();
        let Some(carried) = crate::semantic::std_knowledge::carried_by(&method) else {
            return;
        };
        let origin = match carried {
            Carried::Receiver | Carried::Held => self.origin_of(&call.receiver),
            Carried::HeldOrArgument(position) => {
                let held = self.origin_of(&call.receiver);
                match call.args.iter().nth(position.into()) {
                    Some(argument) => {
                        let fallback = self.origin_of(argument);
                        self.either(span, held, fallback)
                    }
                    None => held
                }
            }
        };
        self.produced.insert(
            (
                u32::try_from(span.start).unwrap_or(u32::MAX),
                u32::try_from(span.end).unwrap_or(u32::MAX)
            ),
            origin
        );
    }

    /// Records the poll an awaited future stands for, where the future's
    /// own type declares one.
    pub(super) fn collect_await(&mut self, awaited: &syn::ExprAwait) {
        if let Some(future_type) = self.called_return(&awaited.base)
            && self.ret(&format!("{future_type}::poll")).is_some()
        {
            self.record_call(Some(&future_type), "poll");
        }
        syn::visit::visit_expr_await(self, awaited);
    }
}
