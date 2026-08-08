// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The types a body constructs, read off the struct literals written in
//! it with `Self` resolved to the type the enclosing impl block names,
//! and what each written field of such a literal was built from.

use super::FactCollector;
use crate::semantic::behavior::types::BuiltField;

impl FactCollector {
    /// Records the type a struct literal constructs, resolving `Self`
    /// through the receiver's own type, and the origin of every written
    /// field.
    ///
    /// The fields are read after the literal's own operands have been
    /// walked, for the same reason call arguments are settled after the
    /// walk: reading them first would name values the walk had not
    /// reached yet.
    pub(super) fn collect_struct_literal(&mut self, literal: &syn::ExprStruct) {
        let constructed = literal.path.segments.last().map(|segment| {
            let name = segment.ident.to_string();
            if name == "Self" {
                self.param_types.get("self").cloned().unwrap_or(name)
            } else {
                name
            }
        });
        if let Some(name) = &constructed {
            self.facts.constructs.push(name.clone());
        }
        syn::visit::visit_expr_struct(self, literal);
        let Some(owner) = constructed else {
            return;
        };
        for field in &literal.fields {
            let syn::Member::Named(name) = &field.member else {
                continue;
            };
            let origin = self.origin_of(&field.expr);
            self.facts.built_fields.push(BuiltField {
                owner: owner.clone(),
                field: name.to_string(),
                origin
            });
        }
    }
}
