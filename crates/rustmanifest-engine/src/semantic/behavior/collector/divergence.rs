// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The calls that do not come back: the process-ending pair of the
//! standard library, and workspace functions whose signature declares
//! the never type.

use super::{naming::expanded_segments, scope::FlowScope};

impl FlowScope<'_> {
    /// Returns whether a call expression names the process-ending pair,
    /// the file's imports applied first — `use std::process::exit` and
    /// `std::process::exit` are the same call and are read the same way.
    pub(super) fn process_exit(&self, callee: &syn::Expr) -> bool {
        let syn::Expr::Path(path) = callee else {
            return false;
        };
        let segments = expanded_segments(self.imports, &path.path);
        matches!(segments.last().map(String::as_str), Some("exit" | "abort"))
            && segments.iter().any(|segment| segment == "process")
    }

    /// Whether a called path names a workspace function whose signature
    /// declares the never type, so the call does not return.
    pub(super) fn never_returns(&self, callee: &syn::Expr) -> bool {
        let syn::Expr::Path(path) = callee else {
            return false;
        };
        let segments = expanded_segments(self.imports, &path.path);
        let Some(bare) = segments.last() else {
            return false;
        };
        let qualified = if segments.len() >= 2 {
            format!("{}::{bare}", segments[segments.len() - 2])
        } else {
            bare.clone()
        };
        [qualified.as_str(), bare.as_str()]
            .iter()
            .filter_map(|key| self.returns.resolve(key, self.src, self.crates))
            .any(|returned| returned.never)
    }
}
