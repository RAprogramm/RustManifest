// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The three shapes a field access takes at a fact site: the target of a
//! plain assignment, the target of a compound one, and a read.

use syn::visit::Visit;

use super::FactCollector;
use crate::semantic::expr::compound_assignment;

impl FactCollector {
    /// Records the target of a plain assignment as a write, then visits
    /// the assigned value.
    pub(super) fn collect_assignment(&mut self, assignment: &syn::ExprAssign) {
        self.record_write_target(&assignment.left);
        self.visit_expr(&assignment.right);
    }

    /// Records the left operand of a compound assignment as a write.
    pub(super) fn collect_binary(&mut self, binary: &syn::ExprBinary) {
        if compound_assignment(binary.op) {
            self.record_write_target(&binary.left);
        }
        syn::visit::visit_expr_binary(self, binary);
        self.record_binary_operation(binary);
    }

    /// Records one field read and the use of the parameter it hangs off.
    pub(super) fn collect_field_access(&mut self, field: &syn::ExprField) {
        if let Some(key) = self.field_key(field) {
            self.facts.reads.push(key);
        }
        if let syn::Expr::Path(path) = field.base.as_ref()
            && let Some(ident) = path.path.get_ident()
        {
            let named = matches!(field.member, syn::Member::Named(_));
            self.bump_param(&ident.to_string(), named);
            self.record_indexed_part(field);
            return;
        }
        syn::visit::visit_expr_field(self, field);
        self.record_indexed_part(field);
    }
}
