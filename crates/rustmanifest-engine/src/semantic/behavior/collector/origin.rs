// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Reading where a written expression's value came from.
//!
//! The reading is total: every shape outside the vocabulary answers with
//! the question it failed on rather than with silence. It records
//! nothing — it is a lookup over what the walk has already built plus the
//! index of a record the walk has already pushed, so every site stays
//! recorded exactly once, by the walk, at the node that owns it.

use super::FactCollector;
use crate::semantic::behavior::value::{Literal, OriginGap, ParamSlot, Provenance};

impl FactCollector {
    /// Where the value of `expression` came from.
    pub(super) fn origin_of(&self, expression: &syn::Expr) -> Provenance {
        match expression {
            syn::Expr::Lit(_) => Literal::of(expression).map_or(
                Provenance::NotDerived(OriginGap::Unreadable),
                Provenance::Written
            ),
            syn::Expr::Reference(borrowed) => self.origin_of(&borrowed.expr),
            syn::Expr::Paren(inner) => self.origin_of(&inner.expr),
            syn::Expr::Group(inner) => self.origin_of(&inner.expr),
            syn::Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Deref(_)) => {
                self.origin_of(&unary.expr)
            }
            syn::Expr::Path(path) => path
                .path
                .get_ident()
                .map_or(Provenance::NotDerived(OriginGap::Foreign), |name| {
                    self.named_origin(&name.to_string())
                }),
            syn::Expr::Field(field) => self.field_origin(field),
            syn::Expr::Return(_) | syn::Expr::Break(_) | syn::Expr::Continue(_) => {
                Provenance::Leaves
            }
            _ => self.recorded_origin(expression)
        }
    }

    /// The field a value is read out of, where the tables name the type
    /// holding it.
    ///
    /// That the value is this field of this type is what the body proves.
    /// What put it there is decided wherever the type is constructed,
    /// which no body of this tier reads — so the answer names the field
    /// and stops, rather than refusing outright or reaching further than
    /// it can see.
    fn field_origin(&self, field: &syn::ExprField) -> Provenance {
        match self.field_key(field) {
            Some(key) if !key.starts_with("?.") => Provenance::Field(key),
            _ => Provenance::NotDerived(OriginGap::Foreign)
        }
    }

    /// Where the value a bare name stands for came from, read off the
    /// name alone.
    pub(super) fn named_value(&self, name: &str) -> Provenance {
        self.named_origin(name)
    }

    /// Where the value a bare name stands for came from.
    ///
    /// A name this body gave a value to answers with that value. A name
    /// the signature declares answers with its own slot, which is the one
    /// free variable a summary is written in terms of. A name written
    /// more than once in this body refuses: which writing stands here is a
    /// question of scopes this tier does not resolve. Anything else
    /// arrives from outside this body and is decided elsewhere.
    fn named_origin(&self, name: &str) -> Provenance {
        if let Some(origin) = self.origins.get(name) {
            return origin.clone();
        }
        if let Some(position) = self
            .slots
            .iter()
            .position(|slot| slot.as_deref() == Some(name))
        {
            return Provenance::Parameter(ParamSlot {
                position: u8::try_from(position).unwrap_or(u8::MAX),
                name:     Some(name.to_owned())
            });
        }
        if self.rebound(name) {
            return Provenance::NotDerived(OriginGap::Rebound);
        }
        Provenance::NotDerived(OriginGap::Foreign)
    }

    /// Whether a name is written in this body without being sole, so no
    /// single value stands behind it.
    fn rebound(&self, name: &str) -> bool {
        self.written.contains(name) && !self.sole.contains(name)
    }

    /// The record the walk pushed for this very expression, by its byte
    /// range, or the question that reading it failed on.
    fn recorded_origin(&self, expression: &syn::Expr) -> Provenance {
        use syn::spanned::Spanned;

        let span = expression.span().byte_range();
        let start = u32::try_from(span.start).unwrap_or(u32::MAX);
        let end = u32::try_from(span.end).unwrap_or(u32::MAX);
        self.produced
            .get(&(start, end))
            .cloned()
            .unwrap_or(Provenance::NotDerived(OriginGap::Unreadable))
    }
}

impl FactCollector {
    /// Records where the value a `let` binds came from, for the one shape
    /// a single value stands behind: a plain name this body writes once.
    ///
    /// A pattern taking a value apart binds several names to parts of it,
    /// and which part each name holds is a question this tier does not
    /// answer; a name written more than once stands for no single value.
    /// Both are left unrecorded, so a later reading of such a name reaches
    /// the refusal that names the question instead of a stale answer.
    ///
    /// A written type is not such a pattern. `let args: Vec<String> = ..`
    /// binds one name to one value exactly as `let args = ..` does, and
    /// reading only the bare form left every annotated binding foreign —
    /// which is most of them in ordinary code.
    pub(super) fn record_local_origin(&mut self, local: &syn::Local) {
        let syn::Pat::Ident(bound) = bound_name(&local.pat) else {
            return;
        };
        if bound.subpat.is_some() {
            return;
        }
        let name = bound.ident.to_string();
        if !self.sole.contains(&name) {
            return;
        }
        let Some(init) = &local.init else {
            return;
        };
        let origin = self.origin_of(&init.expr);
        self.origins.insert(name, origin);
    }
}

/// The pattern under any written type, so an annotated binding reads like
/// the bare one it is.
fn bound_name(pattern: &syn::Pat) -> &syn::Pat {
    match pattern {
        syn::Pat::Type(typed) => bound_name(&typed.pat),
        other => other
    }
}
