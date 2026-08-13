// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The two readings this channel keeps off the written text: the imports
//! a path is spelled against, and the fold that decides what a body
//! reaches.

use super::{FactCollector, naming, scope::FlowScope};

impl FactCollector {
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
