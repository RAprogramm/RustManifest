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

    /// Whether the call written here does not come back.
    ///
    /// The never type is what says so, and the compiler states it as the
    /// type of the call expression itself — the callee inside it is a
    /// function, and its own type is a signature rather than an answer.
    /// Looking the callee up by written name answered only for the
    /// functions this workspace declares, and for those only when the
    /// name was unique.
    pub(in crate::semantic::behavior::collector) fn never_returns(&self, call: &syn::Expr) -> bool {
        use syn::spanned::Spanned;

        let span = call.span().byte_range();
        let (Ok(start), Ok(end)) = (u32::try_from(span.start), u32::try_from(span.end)) else {
            return false;
        };
        self.leaving.contains(&(start, end))
    }
}
