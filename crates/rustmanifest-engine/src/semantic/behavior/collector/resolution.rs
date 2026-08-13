// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The two readings this channel keeps off the written text: the imports
//! a path is spelled against, and the fold that decides what a body
//! reaches.

use super::{FactCollector, naming, scope::FlowScope};

impl FactCollector {
    /// The full path a written path reaches, as the compiler settled it.
    ///
    /// Where nothing was settled the file's own imports are applied to
    /// the written head, which is the same answer for the spellings a
    /// reading can follow and no answer at all for the rest.
    pub(super) fn expanded_segments(&self, path: &syn::Path) -> Vec<String> {
        use syn::spanned::Spanned as _;

        let span = path.span().byte_range();
        if let (Ok(start), Ok(end)) = (u32::try_from(span.start), u32::try_from(span.end))
            && let Some(settled) = self.named.get(&(start, end))
        {
            return settled.split("::").map(ToOwned::to_owned).collect();
        }
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
