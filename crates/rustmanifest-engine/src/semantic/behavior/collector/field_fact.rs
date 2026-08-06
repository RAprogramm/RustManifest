// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Field facts: the `Owner.field` key a field expression renders as, the
//! routing of one field write to a build or to a write, and the use count
//! of the parameter the field hangs off.

use super::FactCollector;

impl FactCollector {
    /// The type a resolver settled for the value a field is read out of.
    ///
    /// A type outside the workspace is one the written text cannot name,
    /// and naming it is the whole of what a field answer needs. The
    /// resolver's answer therefore outranks the reading below, which can
    /// only name what the workspace declares.
    pub(super) fn settled_owner(&self, field: &syn::ExprField) -> Option<String> {
        use syn::spanned::Spanned;

        let span = field.span().byte_range();
        let start = u32::try_from(span.start).ok()?;
        let end = u32::try_from(span.end).ok()?;
        self.held.get(&(start, end)).map(|(owner, _)| owner.clone())
    }

    /// The type a resolver settled for the field read at a position.
    pub(super) fn settled_field_type(&self, field: &syn::ExprField) -> Option<String> {
        use syn::spanned::Spanned;

        let span = field.span().byte_range();
        let start = u32::try_from(span.start).ok()?;
        let end = u32::try_from(span.end).ok()?;
        self.held
            .get(&(start, end))
            .map(|(_, produced)| produced.clone())
    }

    /// Renders a field expression as `Owner.field` where the tables name
    /// the type holding it, or `?.field` where they do not.
    ///
    /// The owner is read by the same typing the rest of the collector
    /// uses, so a field of a field, of an element, or of a local typed by
    /// its own constructor is named exactly where that typing answers.
    /// Reading only a bare parameter name here made the two readings of
    /// one question disagree, and left every nested chain unnamed.
    pub(super) fn field_key(&self, field: &syn::ExprField) -> Option<String> {
        let syn::Member::Named(name) = &field.member else {
            return None;
        };
        let owner = self
            .settled_owner(field)
            .or_else(|| self.expr_type(&field.base));
        Some(format!("{}.{name}", owner.as_deref().unwrap_or("?")))
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
