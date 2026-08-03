// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The visitor over one item body: it proves which values hold a router,
//! tracks how many constructs that may not run enclose the node it is
//! looking at, and hands every framework call on to the recorder.

use std::collections::BTreeSet;

use syn::{Expr, Item, visit::Visit};

use super::{
    attribute::{cfg_gated, statement_attributes},
    binding::{binding_name, router_constructor, router_typed},
    framework::MODELS,
    provider::Provider,
    registration::record,
    table::Surface
};

/// Visitor collecting registrations and compositions from one body.
pub(super) struct Walk<'a> {
    /// Index of the source under analysis.
    pub(super) source:      usize,
    /// Depth of constructs that may not run around the visited node.
    pub(super) conditional: u32,
    /// Local names proven to hold a router value.
    pub(super) routers:     BTreeSet<String>,
    /// Provider being filled.
    pub(super) provider:    &'a mut Provider,
    /// Report counters.
    pub(super) recovered:   &'a mut Surface
}

impl Walk<'_> {
    /// Visits a node with one more construct that may not run around it.
    fn under<F: FnOnce(&mut Self)>(&mut self, body: F) {
        self.conditional += 1;
        body(self);
        self.conditional -= 1;
    }

    /// Whether an expression is proven to evaluate to a router value:
    /// a chain rooted in a router type's constructor, or in a binding
    /// already proven to hold one. Anything else is not assumed.
    fn router_expression(&self, expression: &Expr) -> bool {
        match expression {
            Expr::MethodCall(call) => self.router_expression(&call.receiver),
            Expr::Call(call) => router_constructor(&call.func),
            Expr::Path(path) => path
                .path
                .get_ident()
                .is_some_and(|ident| self.routers.contains(&ident.to_string())),
            Expr::Paren(inner) => self.router_expression(&inner.expr),
            Expr::Group(inner) => self.router_expression(&inner.expr),
            Expr::Reference(inner) => self.router_expression(&inner.expr),
            Expr::Try(inner) => self.router_expression(&inner.expr),
            Expr::Await(inner) => self.router_expression(&inner.base),
            _ => false
        }
    }
}

impl<'ast> Visit<'ast> for Walk<'_> {
    fn visit_item(&mut self, _: &'ast Item) {}

    fn visit_stmt(&mut self, statement: &'ast syn::Stmt) {
        if cfg_gated(statement_attributes(statement)) {
            self.under(|walk| syn::visit::visit_stmt(walk, statement));
            return;
        }
        syn::visit::visit_stmt(self, statement);
    }

    fn visit_local(&mut self, local: &'ast syn::Local) {
        syn::visit::visit_local(self, local);
        let Some(name) = binding_name(&local.pat) else {
            return;
        };
        let proven = router_typed(&local.pat)
            || local
                .init
                .as_ref()
                .is_some_and(|init| self.router_expression(&init.expr));
        if proven {
            self.routers.insert(name);
        }
    }

    fn visit_expr_assign(&mut self, assign: &'ast syn::ExprAssign) {
        syn::visit::visit_expr_assign(self, assign);
        if !self.router_expression(&assign.right) {
            return;
        }
        if let Expr::Path(path) = &*assign.left
            && let Some(ident) = path.path.get_ident()
        {
            self.routers.insert(ident.to_string());
        }
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        let name = call.method.to_string();
        if let Some(model) = MODELS.iter().copied().find(|model| model.knows(&name)) {
            if self.router_expression(&call.receiver) {
                record(model, &name, call, self);
            } else {
                self.recovered.foreign_receiver += 1;
            }
        }
        syn::visit::visit_expr_method_call(self, call);
    }

    fn visit_expr_if(&mut self, conditional: &'ast syn::ExprIf) {
        self.visit_expr(&conditional.cond);
        self.under(|walk| {
            walk.visit_block(&conditional.then_branch);
            if let Some((_, otherwise)) = &conditional.else_branch {
                walk.visit_expr(otherwise);
            }
        });
    }

    fn visit_expr_match(&mut self, matched: &'ast syn::ExprMatch) {
        self.visit_expr(&matched.expr);
        self.under(|walk| {
            for arm in &matched.arms {
                walk.visit_arm(arm);
            }
        });
    }

    fn visit_expr_closure(&mut self, closure: &'ast syn::ExprClosure) {
        self.under(|walk| syn::visit::visit_expr_closure(walk, closure));
    }

    fn visit_expr_for_loop(&mut self, loop_expr: &'ast syn::ExprForLoop) {
        self.visit_expr(&loop_expr.expr);
        self.under(|walk| walk.visit_block(&loop_expr.body));
    }

    fn visit_expr_while(&mut self, loop_expr: &'ast syn::ExprWhile) {
        self.visit_expr(&loop_expr.cond);
        self.under(|walk| walk.visit_block(&loop_expr.body));
    }
}
