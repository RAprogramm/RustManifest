// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The syntax walk itself: which node shapes the collector stops at, each
//! one handing the node to the rule that reads it.

use syn::visit::Visit;

use super::FactCollector;

impl<'ast> Visit<'ast> for FactCollector {
    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        self.record_exit_call(call);
        self.record_called_path(call);
    }

    fn visit_local(&mut self, local: &'ast syn::Local) {
        self.record_local_binding(local);
    }

    fn visit_expr_for_loop(&mut self, loop_expr: &'ast syn::ExprForLoop) {
        self.collect_for_loop(loop_expr);
    }

    fn visit_expr_while(&mut self, loop_expr: &'ast syn::ExprWhile) {
        self.collect_while_loop(loop_expr);
    }

    fn visit_expr_loop(&mut self, loop_expr: &'ast syn::ExprLoop) {
        self.collect_bare_loop(loop_expr);
    }

    fn visit_expr_if(&mut self, conditional: &'ast syn::ExprIf) {
        self.collect_conditional(conditional);
    }

    fn visit_expr_match(&mut self, matched: &'ast syn::ExprMatch) {
        self.collect_match(matched);
    }

    fn visit_expr_closure(&mut self, closure: &'ast syn::ExprClosure) {
        self.collect_closure(closure);
    }

    fn visit_expr_async(&mut self, block: &'ast syn::ExprAsync) {
        self.collect_async_block(block);
    }

    fn visit_item_fn(&mut self, nested: &'ast syn::ItemFn) {
        self.collect_nested_fn(nested);
    }

    fn visit_block(&mut self, block: &'ast syn::Block) {
        self.collect_block_statements(block);
    }

    fn visit_expr_assign(&mut self, assignment: &'ast syn::ExprAssign) {
        self.collect_assignment(assignment);
    }

    fn visit_expr_binary(&mut self, binary: &'ast syn::ExprBinary) {
        self.collect_binary(binary);
    }

    fn visit_expr_field(&mut self, field: &'ast syn::ExprField) {
        self.collect_field_access(field);
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        self.collect_method_call(call);
        self.record_statement_bind(call);
    }

    fn visit_expr_struct(&mut self, literal: &'ast syn::ExprStruct) {
        self.collect_struct_literal(literal);
    }

    fn visit_expr_unsafe(&mut self, block: &'ast syn::ExprUnsafe) {
        self.collect_unsafe_block(block);
    }

    fn visit_expr_try(&mut self, tried: &'ast syn::ExprTry) {
        self.collect_try(tried);
    }

    fn visit_expr_await(&mut self, awaited: &'ast syn::ExprAwait) {
        self.collect_await(awaited);
    }

    fn visit_expr_unary(&mut self, unary: &'ast syn::ExprUnary) {
        syn::visit::visit_expr_unary(self, unary);
        self.record_unary_operation(unary);
    }

    fn visit_expr_index(&mut self, indexed: &'ast syn::ExprIndex) {
        syn::visit::visit_expr_index(self, indexed);
        self.record_selection(indexed);
    }

    fn visit_expr_cast(&mut self, cast: &'ast syn::ExprCast) {
        syn::visit::visit_expr_cast(self, cast);
        self.record_cast(cast);
    }

    fn visit_expr_return(&mut self, returned: &'ast syn::ExprReturn) {
        self.collect_return(returned);
    }

    fn visit_expr_path(&mut self, path: &'ast syn::ExprPath) {
        self.record_exit_constant(path);
        self.collect_path(path);
    }

    fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
        self.collect_macro(invocation);
    }
}
