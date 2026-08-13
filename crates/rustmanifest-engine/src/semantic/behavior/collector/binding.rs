// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a `let` teaches the tables: where the bound value came from, the
//! written type of the binding, the element type of a collection it
//! declares — or of the sequence a call hands it — the wrapped inner
//! type it inherits, the process stream it holds a handle to, and
//! whether the value it holds was constructed in this body.

use super::{FactCollector, typing::initializer_type};

impl FactCollector {
    /// Records the types and the freshness a `let` binding carries.
    ///
    /// Every name the `let` binds is typed by what the compiler settled
    /// at the name itself, however deeply the pattern destructures. A
    /// written annotation used to answer for the simple case, and it
    /// answered with the last segment somebody spelled — through an
    /// alias, through an import renaming it, and only where an annotation
    /// was written at all.
    pub(super) fn record_local_binding(&mut self, local: &syn::Local) {
        self.bind_pattern(&local.pat);
        let name = match &local.pat {
            syn::Pat::Ident(pattern) => Some(pattern.ident.to_string()),
            syn::Pat::Type(typed) => match typed.pat.as_ref() {
                syn::Pat::Ident(pattern) => Some(pattern.ident.to_string()),
                _ => None
            },
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
