// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Where the scan meets the syntax tree: the node kinds that carry
//! evidence — function items, impl blocks, calls, method calls and bare
//! paths.

use syn::visit::Visit;

use super::Scan;

impl<'ast> Visit<'ast> for Scan<'_> {
    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        if !self.collect_calls && self.owner.is_none() && item.sig.ident == self.bare {
            self.record_definition(&item.sig);
        }
        syn::visit::visit_item_fn(self, item);
    }

    fn visit_item_impl(&mut self, item: &'ast syn::ItemImpl) {
        let name = match item.self_ty.as_ref() {
            syn::Type::Path(path) => path.path.segments.last().map(|s| s.ident.to_string()),
            _ => None
        };
        for entry in &item.items {
            if let syn::ImplItem::Fn(method) = entry
                && method.sig.ident == self.bare
                && !self.collect_calls
            {
                if item.trait_.is_none()
                    && self
                        .owner
                        .is_some_and(|owner| Some(owner.to_owned()) == name)
                {
                    self.record_definition(&method.sig);
                } else {
                    self.definitions.push(self.source_index);
                }
            }
        }
        syn::visit::visit_item_impl(self, item);
    }

    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = call.func.as_ref()
            && path
                .path
                .segments
                .last()
                .is_some_and(|segment| segment.ident == self.bare)
        {
            if self.collect_calls {
                self.record_call(&call.args);
            }
            for argument in &call.args {
                self.visit_expr(argument);
            }
            return;
        }
        syn::visit::visit_expr_call(self, call);
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        if self.collect_calls && call.method == self.bare && self.owner.is_some() {
            self.record_call(&call.args);
        }
        syn::visit::visit_expr_method_call(self, call);
    }

    fn visit_expr_path(&mut self, path: &'ast syn::ExprPath) {
        if self.collect_calls
            && path
                .path
                .segments
                .last()
                .is_some_and(|segment| segment.ident == self.bare)
        {
            self.bare_path_refs += 1;
        }
        syn::visit::visit_expr_path(self, path);
    }
}
