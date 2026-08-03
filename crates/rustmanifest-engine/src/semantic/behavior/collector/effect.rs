// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Side-effect categories and allocations read off a written path, and
//! the effect an `unsafe` block marks the function with.

use super::FactCollector;
use crate::semantic::{behavior::types::Effect, knowledge};

impl FactCollector {
    /// Classifies a call path against the effect sink patterns, seen
    /// through the file's imports.
    pub(super) fn record_path_effect(&mut self, path: &syn::Path) {
        let segments = self.expanded_segments(path);
        let joined = segments.join("::");
        let effect = if joined.contains("fs::") || segments.first().is_some_and(|s| s == "File") {
            Some(Effect::FileSystem)
        } else if joined.contains("net::")
            || segments
                .iter()
                .any(|s| s == "TcpStream" || s == "TcpListener" || s == "UdpSocket")
        {
            Some(Effect::Network)
        } else if joined.contains("process::") || segments.first().is_some_and(|s| s == "Command")
        {
            Some(Effect::Process)
        } else if joined.contains("env::") {
            Some(Effect::Environment)
        } else if segments.len() == 2 {
            segments
                .first()
                .and_then(|first| knowledge::crate_effect(first))
        } else {
            None
        };
        if let Some(effect) = effect {
            self.facts.effects.push(effect);
        }
    }

    /// Records the effects and allocations a written path carries, and
    /// counts a bare name as one use of the parameter it may name.
    pub(super) fn collect_path(&mut self, path: &syn::ExprPath) {
        if path.path.segments.len() >= 2 {
            self.record_path_effect(&path.path);
            let first = path.path.segments[0].ident.to_string();
            let last = path.path.segments.last().map(|s| s.ident.to_string());
            if matches!(
                (first.as_str(), last.as_deref()),
                (
                    "Box" | "Vec" | "String" | "Rc" | "Arc",
                    Some("new" | "from" | "with_capacity")
                )
            ) {
                self.facts.allocations += 1;
            }
        } else if let Some(ident) = path.path.get_ident() {
            let name = ident.to_string();
            if self.imports.contains_key(&name) {
                self.record_path_effect(&path.path);
            }
            self.bump_param(&name, false);
        }
        syn::visit::visit_expr_path(self, path);
    }

    /// Marks the function as carrying the unsafe effect.
    pub(super) fn collect_unsafe_block(&mut self, block: &syn::ExprUnsafe) {
        self.facts.effects.push(Effect::Unsafe);
        syn::visit::visit_expr_unsafe(self, block);
    }
}
