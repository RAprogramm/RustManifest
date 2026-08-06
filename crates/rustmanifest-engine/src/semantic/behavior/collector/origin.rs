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
            syn::Expr::Path(path) => self.path_origin(&path.path),
            syn::Expr::Field(field) => self.field_origin(field),
            syn::Expr::Try(fallible) => self.propagated_origin(fallible),
            syn::Expr::Block(block) => self.block_origin(&block.block),
            syn::Expr::Unsafe(block) => self.block_origin(&block.block),
            syn::Expr::Await(awaited) => self.origin_of(&awaited.base),
            syn::Expr::Array(_) | syn::Expr::Repeat(_) => {
                Provenance::Constructed("array".to_owned())
            }
            syn::Expr::Tuple(_) => Provenance::Constructed("tuple".to_owned()),
            syn::Expr::Closure(_) => Provenance::Constructed("closure".to_owned()),
            syn::Expr::Range(_) => Provenance::Constructed("range".to_owned()),
            syn::Expr::Macro(invocation) => {
                crate::semantic::std_knowledge::macro_value(&invocation.mac.path).map_or_else(
                    || self.recorded_origin(expression),
                    |built| Provenance::Constructed(built.to_owned())
                )
            }
            syn::Expr::Struct(literal) => literal.path.segments.last().map_or_else(
                || Provenance::NotDerived(OriginGap::Unreadable),
                |segment| Provenance::Constructed(segment.ident.to_string())
            ),
            syn::Expr::Return(_) | syn::Expr::Break(_) | syn::Expr::Continue(_) => {
                Provenance::Leaves
            }
            _ => self.recorded_origin(expression)
        }
    }

    /// Where the value a written path stands for came from.
    ///
    /// A bare name is read as a binding of this body first, because a
    /// body's own name is what the body proves. A path the library
    /// tables answer for stands for the value it names, which is settled
    /// wherever it is written. Anything else arrives from outside this
    /// body and is decided elsewhere.
    fn path_origin(&self, path: &syn::Path) -> Provenance {
        if let Some(name) = path.get_ident() {
            let bound = self.named_origin(&name.to_string());
            if bound.derived() {
                return bound;
            }
        }
        if let Some(named) = crate::semantic::std_knowledge::value_path(path) {
            return Provenance::Constant(named);
        }
        if let Some(named) = self.declared_path(path) {
            return Provenance::Constant(named);
        }
        if let Some(named) = self.function_path(path) {
            return Provenance::Constant(named);
        }
        path.get_ident()
            .map_or(Provenance::NotDerived(OriginGap::Foreign), |name| {
                self.named_origin(&name.to_string())
            })
    }

    /// The name a written path stands for, where the workspace declares
    /// it as a value: a constant, a static, or a variant carrying
    /// nothing.
    ///
    /// The qualified spelling is asked first, so a bare name two
    /// declarations share answers for neither rather than for the nearest
    /// one.
    fn declared_path(&self, path: &syn::Path) -> Option<String> {
        let segments: Vec<String> = path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect();
        let written = segments.join("::");
        if let [.., owner, named] = segments.as_slice()
            && self
                .declared
                .resolve(&format!("{owner}.{named}"), self.src, &self.crates)
                .is_some()
        {
            return Some(written);
        }
        let named = segments.last()?;
        self.declared
            .resolve(named, self.src, &self.crates)
            .map(|()| written)
    }

    /// The value a propagating operator leaves standing where it does not
    /// leave the function.
    ///
    /// `x?` and `x` stand for the same value: the operator removes the
    /// failure half and hands on the success half, and which value that
    /// is was decided by whatever produced `x`. The propagation itself is
    /// already a fact of its own — the disposal recorded around the site
    /// says the failure leaves — so reading the origin through it states
    /// nothing the site does not prove.
    fn propagated_origin(&self, fallible: &syn::ExprTry) -> Provenance {
        self.origin_of(&fallible.expr)
    }

    /// Awaiting and an unsafe block change nothing about which value
    /// stands at the position: the first waits for the value its own
    /// expression stands for, the second only lifts a checking rule. Both
    /// are seen through, exactly as a borrow and a parenthesis are.
    ///
    /// The value a block stands for: the value of the expression it ends
    /// with, where it ends with one.
    ///
    /// A block ending in an expression stands for exactly that
    /// expression's value; a block ending any other way produces the unit
    /// value, and the language guarantees both. The one exception is a
    /// block whose last statement leaves the function: nothing is
    /// produced there at all, which is a settled answer of its own and
    /// not the unit value.
    pub(super) fn block_value(&self, block: &syn::Block) -> Provenance {
        self.block_origin(block)
    }

    /// The value a block stands for, as the reading itself uses it.
    fn block_origin(&self, block: &syn::Block) -> Provenance {
        let Some(syn::Stmt::Expr(last, terminator)) = block.stmts.last() else {
            return Provenance::Written(Literal::Unit);
        };
        let leaves = matches!(
            last,
            syn::Expr::Return(_) | syn::Expr::Break(_) | syn::Expr::Continue(_)
        );
        if terminator.is_none() || leaves {
            return self.origin_of(last);
        }
        Provenance::Written(Literal::Unit)
    }

    /// The name a written path stands for, where the workspace declares
    /// a function under it.
    ///
    /// A function written without being called is a value: it is handed
    /// to an adapter, stored, or passed on, and which function it is, is
    /// exactly what the path says. Only paths the return table answers
    /// for are read this way, so a name the workspace never declares
    /// stays the refusal it was.
    fn function_path(&self, path: &syn::Path) -> Option<String> {
        let segments: Vec<String> = path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect();
        let written = segments.join("::");
        let qualified = match segments.as_slice() {
            [.., owner, named] => Some(format!("{owner}::{named}")),
            _ => None
        };
        if qualified
            .as_deref()
            .is_some_and(|key| self.ret(key).is_some())
        {
            return Some(written);
        }
        let [only] = segments.as_slice() else {
            return None;
        };
        self.ret(only).map(|_| written)
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
    /// question of scopes this tier does not resolve.
    ///
    /// A name nothing in the body binds, which the workspace declares as
    /// a constant or a static, is that constant: the declaration proves
    /// what the name stands for, and where it is declared settles what it
    /// holds. The order matters — a binding of the same name shadows the
    /// declaration, so the body is asked first. Anything else arrives
    /// from outside this body and is decided elsewhere.
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
        if self
            .declared
            .resolve(name, self.src, &self.crates)
            .is_some()
        {
            return Provenance::Constant(name.to_owned());
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
    /// A tuple pattern is the one shape that answers which part each name
    /// holds — the name at position `k` holds the element at position `k`
    /// — and each part is recorded as the selection it is. Every other
    /// pattern taking a value apart, and every name written more than
    /// once, is left unrecorded, so a later reading reaches the refusal
    /// that names the question instead of a stale answer.
    ///
    /// A written type is not such a pattern. `let args: Vec<String> = ..`
    /// binds one name to one value exactly as `let args = ..` does, and
    /// reading only the bare form left every annotated binding foreign —
    /// which is most of them in ordinary code.
    pub(super) fn record_local_origin(&mut self, local: &syn::Local) {
        use syn::spanned::Spanned;

        let pattern = bound_name(&local.pat);
        if matches!(pattern, syn::Pat::Tuple(_) | syn::Pat::TupleStruct(_)) {
            let Some(init) = &local.init else {
                return;
            };
            let taken = self.origin_of(&init.expr);
            self.record_tuple_parts(pattern, &taken, &local.pat.span().byte_range());
            return;
        }
        let syn::Pat::Ident(bound) = pattern else {
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
