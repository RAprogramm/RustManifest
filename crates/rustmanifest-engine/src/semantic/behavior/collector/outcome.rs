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

/// The two variants of the library's result, with the side of the
/// signature each one carries.
const RESULT_VARIANTS: [(&str, bool); 2] = [("Ok", true), ("Err", false)];

impl FactCollector {
    /// Records the type each arm of a match over a result binds.
    pub(super) fn record_outcome_bindings(&mut self, matched: &syn::ExprMatch) {
        let Some(key) = self.call_key(&matched.expr) else {
            return;
        };
        let Some(returned) = self.ret(&key) else {
            return;
        };
        let (success, error) = (returned.success.clone(), returned.error.clone());
        if success.is_none() && error.is_none() {
            return;
        }
        for arm in &matched.arms {
            let Some((variant, bound)) = variant_binding(&arm.pat) else {
                continue;
            };
            let carried = RESULT_VARIANTS
                .iter()
                .find(|(named, _)| *named == variant)
                .and_then(|(_, ok)| if *ok { success.clone() } else { error.clone() });
            if variant == "Ok" {
                let produced = self.origin_of(&matched.expr);
                self.origins.insert(bound.clone(), produced);
            }
            if let Some(type_name) = carried {
                self.param_types.insert(bound, type_name);
            }
        }
    }
}

impl FactCollector {
    /// Records what a `let` written in a condition binds, on the same
    /// ground the arms of a match over the same value are read.
    ///
    /// `if let Ok(value) = call()` and the `Ok` arm of a match over that
    /// call bind the same name to the same half of the same value. They
    /// are one fact written two ways, so they are read by one rule — the
    /// second spelling was left refusing only because nothing looked at
    /// it.
    pub(super) fn record_condition_binding(&mut self, condition: &syn::Expr) {
        let syn::Expr::Let(written) = condition else {
            return;
        };
        let Some((variant, bound)) = variant_binding(&written.pat) else {
            return;
        };
        let Some(key) = self.call_key(&written.expr) else {
            return;
        };
        let Some(returned) = self.ret(&key) else {
            return;
        };
        if let Some(type_name) = RESULT_VARIANTS
            .iter()
            .find(|(named, _)| *named == variant)
            .and_then(|(_, ok)| {
                if *ok {
                    returned.success.clone()
                } else {
                    returned.error.clone()
                }
            })
        {
            self.param_types.insert(bound.clone(), type_name);
        }
        if variant == "Ok" {
            let produced = self.origin_of(&written.expr);
            self.origins.insert(bound, produced);
        }
    }
}

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
