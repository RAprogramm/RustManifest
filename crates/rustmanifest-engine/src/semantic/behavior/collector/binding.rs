// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a `let` teaches the tables: where the bound value came from, the
//! written type of the binding, the element type of a collection it
//! declares — or of the sequence a call hands it — the wrapped inner
//! type it inherits, the process stream it holds a handle to, and
//! whether the value it holds was constructed in this body.

use super::{FactCollector, typing::initializer_type};
use crate::semantic::behavior::signature::type_last_segment;

impl FactCollector {
    /// Types every name a destructuring `let` binds from what a resolver
    /// settled at each binding.
    ///
    /// Which type stands at which position is the compiler's answer at
    /// the name itself; reading it off the callee's declared tuple was
    /// the same answer taken the long way round, and only for a callee
    /// this workspace declares.
    fn record_tuple_binding(&mut self, local: &syn::Local) {
        if matches!(local.pat, syn::Pat::Tuple(_)) {
            self.bind_pattern(&local.pat);
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
