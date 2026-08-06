// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Facts read off an invocation: the callee a path call names, everything
//! a method call carries — the writes it performs, the clones and the
//! output it produces, the bindings it types and the effects it reaches —
//! and the poll an awaited future stands for.
//!
//! Two questions of the reading live beside this one: [`carrying`] hands
//! on the origin a call carries instead of deriving, and [`walk`] settles
//! the order a call's own values are read in.

mod carrying;
mod walk;

use super::{
    FactCollector,
    catalog::{BYTE_WRITE_METHOD, ITERATOR_ADAPTERS, TEXT_WRITE_METHOD}
};
use crate::semantic::{expr::MUTATING_METHODS, knowledge};

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
        let reserved = self.reserve_method_call(call, &method, &span);
        if ITERATOR_ADAPTERS.contains(&method.as_str()) {
            self.scoped(|collector| {
                collector.bind_adapter_inputs(call, &span);
                collector.walk_method_call(call);
            });
        } else {
            self.walk_method_call(call);
        }
        let mut written: Vec<&syn::Expr> = vec![call.receiver.as_ref()];
        written.extend(call.args.iter());
        self.settle_arguments(reserved, &written);
        if self.record_read_method(call).is_none() && !self.follow_intake(call) {
            self.record_method_operation(call);
        }
        self.carry_outcome(call, &span);
    }

    /// Reserves the record one method call fills, and records the effect
    /// the tables state its receiver's type reaches.
    fn reserve_method_call(
        &mut self,
        call: &syn::ExprMethodCall,
        method: &str,
        span: &std::ops::Range<usize>
    ) -> usize {
        let Some(owner) = self.expr_type(&call.receiver) else {
            return self.reserve_written_call(Some("?"), method, span);
        };
        if let Some(effect) = knowledge::method_effect(&owner, method)
            .or_else(|| crate::semantic::std_knowledge::method_effect(&owner, method))
        {
            self.facts.effects.push(effect);
        }
        self.reserve_written_call(Some(&owner), method, span)
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
