// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Field facts: the `Owner.field` key a field expression renders as, the
//! routing of one field write to a build or to a write, and the use count
//! of the parameter the field hangs off.

use super::FactCollector;

impl FactCollector {
    /// Renders a field expression as `Owner.field` when the base is a
    /// typed parameter or `self`, or `?.field` otherwise.
    pub(super) fn field_key(&self, field: &syn::ExprField) -> Option<String> {
        let syn::Member::Named(name) = &field.member else {
            return None;
        };
        let owner = match field.base.as_ref() {
            syn::Expr::Path(path) => path
                .path
                .get_ident()
                .and_then(|ident| self.param_types.get(&ident.to_string()))
                .map_or("?", String::as_str),
            _ => "?"
        };
        Some(format!("{owner}.{name}"))
    }

    /// Records the outermost field of an assignment target as a write —
    /// or as a build when the base is a freshly constructed local — and
    /// counts the base parameter's use so a written-through parameter
    /// never looks read-only.
    pub(super) fn record_write_target(&mut self, target: &syn::Expr) {
        if let syn::Expr::Field(field) = target {
            if let Some(key) = self.field_key(field) {
                self.record_field_write(field, key);
            }
            if let syn::Expr::Path(path) = field.base.as_ref()
                && let Some(ident) = path.path.get_ident()
            {
                self.bump_param(&ident.to_string(), false);
            }
        }
    }

    /// Routes one field write to `builds` when its base binding holds a
    /// value constructed in this body, to `writes` otherwise.
    pub(super) fn record_field_write(&mut self, field: &syn::ExprField, key: String) {
        let fresh_base = matches!(
            field.base.as_ref(),
            syn::Expr::Path(path) if path
                .path
                .get_ident()
                .is_some_and(|ident| self.fresh.contains(&ident.to_string()))
        );
        if fresh_base {
            self.facts.builds.push(key);
        } else {
            self.facts.writes.push(key);
        }
    }

    /// Counts one use of the parameter named `name`; `field_read` marks a
    /// use that is only a field read.
    pub(super) fn bump_param(&mut self, name: &str, field_read: bool) {
        if let Some(param) = self
            .facts
            .params
            .iter_mut()
            .find(|param| param.name == name)
        {
            param.uses += 1;
            if field_read {
                param.field_reads += 1;
            }
        }
    }
}
