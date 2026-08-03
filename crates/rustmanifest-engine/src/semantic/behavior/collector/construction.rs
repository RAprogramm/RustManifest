// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The types a body constructs, read off the struct literals written in
//! it with `Self` resolved to the type the enclosing impl block names.

use super::FactCollector;

impl FactCollector {
    /// Records the type a struct literal constructs, resolving `Self`
    /// through the receiver's own type.
    pub(super) fn collect_struct_literal(&mut self, literal: &syn::ExprStruct) {
        if let Some(segment) = literal.path.segments.last() {
            let name = segment.ident.to_string();
            let constructed = if name == "Self" {
                self.param_types.get("self").cloned().unwrap_or(name)
            } else {
                name
            };
            self.facts.constructs.push(constructed);
        }
        syn::visit::visit_expr_struct(self, literal);
    }
}
