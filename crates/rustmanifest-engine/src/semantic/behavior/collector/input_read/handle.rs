// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a receiver of a read draws from: the library type an initializer
//! produces, and the channel a written handle leads to.
//!
//! The mirror of the reading the write channel performs on an output
//! handle, kept beside it for the same purpose: a binding holding a
//! handle names its channel without a written type, and a handle
//! constructed at the site names it without a binding at all.

use super::{
    super::{
        super::input::Channel,
        catalog::{
            STREAM_GUARD_SUFFIX, STREAM_LOCK_METHOD,
            intake::{INTAKE_HANDLE_PATHS, INTAKE_TYPES}
        }
    },
    FactCollector
};
use crate::semantic::behavior::output::Acquisition;

impl FactCollector {
    /// The library type an initializer produces, where that type is the
    /// process's own input handle or the guard its locking call hands
    /// out.
    ///
    /// The mirror of the reading the write channel performs on an output
    /// handle, and kept beside it for the same purpose: a binding holding
    /// a handle names its channel without a written type.
    pub(in crate::semantic::behavior::collector) fn intake_type(
        &self,
        expression: &syn::Expr
    ) -> Option<&'static str> {
        match expression {
            syn::Expr::Reference(borrowed) => self.intake_type(&borrowed.expr),
            syn::Expr::Paren(inner) => self.intake_type(&inner.expr),
            syn::Expr::Group(inner) => self.intake_type(&inner.expr),
            syn::Expr::MethodCall(call) if call.method == STREAM_LOCK_METHOD => {
                let handle = self.intake_type(&call.receiver)?;
                INTAKE_HANDLE_PATHS
                    .iter()
                    .find(|(_, _, named, _)| *named == handle)
                    .map(|(_, _, _, guard)| *guard)
            }
            syn::Expr::Call(call) => {
                let syn::Expr::Path(path) = call.func.as_ref() else {
                    return None;
                };
                let segments = self.expanded_segments(&path.path);
                INTAKE_HANDLE_PATHS
                    .iter()
                    .find(|(named, _, _, _)| {
                        named.len() == segments.len() && matched(named, &segments)
                    })
                    .map(|(_, _, named, _)| *named)
            }
            syn::Expr::Path(path) => {
                let name = path.path.get_ident()?.to_string();
                let written = self.param_types.get(&name)?;
                INTAKE_TYPES
                    .iter()
                    .find(|(named, _)| *named == written.as_str())
                    .map(|(named, _)| *named)
            }
            _ => None
        }
    }

    /// The channel a receiver draws from, and where its lock was taken.
    pub(super) fn intake_channel(&self, receiver: &syn::Expr) -> Option<(Channel, Acquisition)> {
        if let Some(channel) = self.constructed_intake(receiver) {
            return Some((channel, Acquisition::AtSite));
        }
        let syn::Expr::Path(path) = receiver else {
            return None;
        };
        let name = path.path.get_ident()?.to_string();
        let written = self.param_types.get(&name)?;
        let channel = INTAKE_TYPES
            .iter()
            .find(|(named, _)| *named == written.as_str())
            .map(|(_, channel)| *channel)?;
        let held = written.ends_with(STREAM_GUARD_SUFFIX);
        Some((
            channel,
            if held {
                Acquisition::Held
            } else {
                Acquisition::AtSite
            }
        ))
    }

    /// The channel a handle constructor written at this site leads to,
    /// seen through the locking call that hands out a guard over it.
    pub(super) fn constructed_intake(&self, expression: &syn::Expr) -> Option<Channel> {
        match expression {
            syn::Expr::Reference(borrowed) => self.constructed_intake(&borrowed.expr),
            syn::Expr::Paren(inner) => self.constructed_intake(&inner.expr),
            syn::Expr::Group(inner) => self.constructed_intake(&inner.expr),
            syn::Expr::MethodCall(call) => self.constructed_intake(&call.receiver),
            syn::Expr::Call(call) => {
                let syn::Expr::Path(path) = call.func.as_ref() else {
                    return None;
                };
                let segments = self.expanded_segments(&path.path);
                INTAKE_HANDLE_PATHS
                    .iter()
                    .find(|(named, _, _, _)| {
                        named.len() == segments.len() && matched(named, &segments)
                    })
                    .map(|(_, channel, _, _)| *channel)
            }
            _ => None
        }
    }
}

/// Whether an expanded path is the catalogued one, segment by segment.
pub(in crate::semantic::behavior::collector) fn matched(
    path: &[&str],
    segments: &[String]
) -> bool {
    path.iter()
        .zip(segments)
        .all(|(named, segment)| *named == segment.as_str())
}
