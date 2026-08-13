// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Scoped lookup: every table this channel reads is keyed by name and
//! resolved against the source the function under analysis sits in, so
//! two crates spelling the same name do not answer for each other.

use super::{FactCollector, naming, scope::FlowScope};

impl FactCollector {
    /// Resolves a field-type key for this function's scope.
    pub(super) fn field(&self, key: &str) -> Option<&String> {
        self.field_types.resolve(key, self.src, &self.crates)
    }

    /// Resolves a wrapper inner-type key for this function's scope.
    pub(super) fn wrap(&self, key: &str) -> Option<&String> {
        self.field_wraps.resolve(key, self.src, &self.crates)
    }

    /// Renders a path as segment names, the first segment substituted
    /// with its full `use` path when the file imported it — so a bare
    /// `remove_dir_all` imported from `std::fs` classifies as a
    /// filesystem sink.
    pub(super) fn expanded_segments(&self, path: &syn::Path) -> Vec<String> {
        naming::expanded_segments(&self.imports, path)
    }

    /// The reachability fold read against this function's own file.
    pub(super) const fn flow_scope(&self) -> FlowScope<'_> {
        FlowScope {
            imports: &self.imports,
            leaving: &self.leaving
        }
    }
}
