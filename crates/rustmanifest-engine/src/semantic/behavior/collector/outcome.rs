// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a match over a result teaches about the names its arms bind.
//!
//! A signature that returns a result names two types, and a match over a
//! call to it hands each of them to one arm. The name an arm binds
//! therefore has a type the source already wrote — in the callee's
//! signature rather than at the binding — and reading it there is a
//! lookup, not an inference.
//!
//! The name an arm of the success side binds also holds the value the
//! scrutinee produced, so a later reading of it reaches through to what
//! the call handed back. The failure side binds the other half, which no
//! term here names, and is left alone.
//!
//! Only the two shapes the library's own result has are read, and only
//! where the arm binds exactly one plain name. A pattern that destructures
//! further, renames through a nested pattern, or names a type of the
//! program's own teaches nothing here and is left alone.

use super::FactCollector;

impl FactCollector {
    /// Records the type each arm of a match over a result binds.
    pub(super) fn record_present_bindings(&mut self, matched: &syn::ExprMatch) {
        for arm in &matched.arms {
            self.bind_pattern(&arm.pat);
        }
    }

    /// Records the type each arm of a match over a result binds.
    pub(super) fn record_outcome_bindings(&mut self, matched: &syn::ExprMatch) {
        for arm in &matched.arms {
            self.bind_pattern(&arm.pat);
            if let Some(("Ok", bound)) = variant_binding(&arm.pat)
                .as_ref()
                .map(|(variant, bound)| (variant.as_str(), bound.clone()))
            {
                let produced = self.origin_of(&matched.expr);
                self.origins.insert(bound, produced);
            }
        }
    }}

impl FactCollector {
    /// Records what a `let` written in a condition binds, on the same
    /// ground the arms of a match over the same value are read.
    ///
    /// A condition may bind more than once: the language chains them with
    /// `&&`, and each link binds in the branch the whole condition
    /// guards, so every link is read.
    ///
    /// `if let Ok(value) = call()` and the `Ok` arm of a match over that
    /// call bind the same name to the same half of the same value. They
    /// are one fact written two ways, so they are read by one rule — the
    /// second spelling was left refusing only because nothing looked at
    /// it.
    pub(super) fn record_condition_binding(&mut self, condition: &syn::Expr) {
        use syn::spanned::Spanned;

        let written = match condition {
            syn::Expr::Let(written) => written,
            syn::Expr::Paren(inner) => {
                self.record_condition_binding(&inner.expr);
                return;
            }
            syn::Expr::Binary(chain) if matches!(chain.op, syn::BinOp::And(_)) => {
                self.record_condition_binding(&chain.left);
                self.record_condition_binding(&chain.right);
                return;
            }
            _ => return
        };
        let taken = self.origin_of(&written.expr);
        self.record_tuple_parts(&written.pat, &taken, &written.pat.span().byte_range());
        self.bind_pattern(&written.pat);
        if let Some((variant, bound)) = variant_binding(&written.pat)
            && variant == "Ok"
        {
            let produced = self.origin_of(&written.expr);
            self.origins.insert(bound, produced);
        }
    }}

/// The variant an arm matches and the single plain name it binds.
fn variant_binding(pattern: &syn::Pat) -> Option<(String, String)> {
    let syn::Pat::TupleStruct(tuple) = pattern else {
        return None;
    };
    let variant = tuple.path.segments.last()?.ident.to_string();
    let [only] = tuple.elems.iter().collect::<Vec<_>>()[..] else {
        return None;
    };
    let syn::Pat::Ident(bound) = only else {
        return None;
    };
    if bound.subpat.is_some() {
        return None;
    }
    Some((variant, bound.ident.to_string()))
}
