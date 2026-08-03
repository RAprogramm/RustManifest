// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Identifiers a loop introduces: the names bound by the loop pattern and
//! by every pattern inside the loop body.
//!
//! A clone whose receiver root is bound inside the loop changes every
//! iteration, so it is not loop-invariant. Origin classification asks this
//! module for that set.

use syn::visit::Visit;

/// Visitor collecting every identifier bound by a pattern in a block.
pub(super) struct BoundCollector<'out> {
    /// Accumulated bound identifiers.
    pub(super) bound: &'out mut Vec<String>
}

impl<'ast> Visit<'ast> for BoundCollector<'_> {
    fn visit_pat_ident(&mut self, pattern: &'ast syn::PatIdent) {
        self.bound.push(pattern.ident.to_string());
        syn::visit::visit_pat_ident(self, pattern);
    }
}

/// Collects binding idents introduced by a pattern.
pub(super) fn pattern_idents(pattern: &syn::Pat, out: &mut Vec<String>) {
    let mut collector = BoundCollector {
        bound: out
    };
    collector.visit_pat(pattern);
}
