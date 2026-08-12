// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a `let` teaches the tables: where the bound value came from, the
//! written type of the binding, the element type of a collection it
//! declares — or of the sequence a call hands it — the wrapped inner
//! type it inherits, the process stream it holds a handle to, and
//! whether the value it holds was constructed in this body.

use super::{FactCollector, typing::initializer_type};
use crate::semantic::behavior::signature::{element_type, pair_types, type_last_segment};

impl FactCollector {
    /// Types every name a destructuring `let` binds from a call that
    /// hands back a tuple, position by position — which name stands at
    /// which position is written, and the return's own type list
    /// answers the rest.
    fn record_tuple_binding(&mut self, local: &syn::Local) {
        let syn::Pat::Tuple(pattern) = &local.pat else {
            return;
        };
        let Some(positions) = local
            .init
            .as_ref()
            .and_then(|init| self.call_key(&init.expr))
            .and_then(|key| self.ret(&key))
            .map(|returned| returned.tuple.clone())
        else {
            return;
        };
        if positions.len() != pattern.elems.len() {
            return;
        }
        for (held, type_name) in pattern.elems.iter().zip(positions) {
            if let Some(name) = super::naming::binding_name(held) {
                self.param_types.insert(name, type_name);
            }
        }
    }

    /// Records the types and the freshness a `let` binding carries.
    pub(super) fn record_local_binding(&mut self, local: &syn::Local) {
        self.record_tuple_binding(local);
        let name = match &local.pat {
            syn::Pat::Ident(pattern) => Some(pattern.ident.to_string()),
            syn::Pat::Type(typed) => {
                if let syn::Pat::Ident(pattern) = typed.pat.as_ref() {
                    if let Some(type_name) = type_last_segment(&typed.ty) {
                        self.param_types
                            .insert(pattern.ident.to_string(), type_name);
                    }
                    if let Some(element) = element_type(&typed.ty) {
                        self.elem_types.insert(pattern.ident.to_string(), element);
                    }
                    if let Some(pair) = pair_types(&typed.ty) {
                        self.local_pairs.insert(pattern.ident.to_string(), pair);
                    }
                    Some(pattern.ident.to_string())
                } else {
                    None
                }
            }
            _ => None
        };
        if let (Some(name), init) = (name.clone(), &local.init)
            && let Some(inner) = init.as_ref().and_then(|init| self.wrapped_of(&init.expr))
        {
            self.local_wraps.insert(name, inner);
        }
        if let Some(name) = name.clone() {
            match local
                .init
                .as_ref()
                .and_then(|init| self.buffered_stream_of(&init.expr))
            {
                Some(stream) => {
                    self.buffered_streams.insert(name, stream);
                }
                None => {
                    self.buffered_streams.remove(&name);
                }
            }
        }
        if let (Some(name), init) = (name, &local.init) {
            let constructed = init.as_ref().and_then(|init| {
                self.handle_type(&init.expr)
                    .or_else(|| self.intake_type(&init.expr))
                    .map(ToOwned::to_owned)
                    .or_else(|| initializer_type(&init.expr))
                    .or_else(|| self.called_return(&init.expr))
                    .or_else(|| self.opened_of(&init.expr))
            });
            if let Some(element) = init
                .as_ref()
                .and_then(|init| self.call_key(&init.expr))
                .and_then(|key| self.ret(&key))
                .and_then(|returned| returned.element.clone())
            {
                self.elem_types.insert(name.clone(), element);
            }
            match constructed {
                Some(type_name) => {
                    self.param_types.insert(name.clone(), type_name);
                    self.fresh.insert(name);
                }
                None => {
                    self.fresh.remove(&name);
                }
            }
        }
        syn::visit::visit_local(self, local);
        self.record_local_origin(local);
    }
}
