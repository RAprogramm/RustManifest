// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Standard macros that leave the function they are written in.
//!
//! What a macro builds is the type a resolver settles for the position, so
//! no list of macros states it here any more. What a macro *does to
//! control flow* is a different fact: a panic leaves the function whatever
//! its type would have been, and no type answers that.
//!
//! Only macros of the standard library are listed, matched on the last
//! segment of the written path. A macro the program defines itself is
//! absent, and stays a refusal: its expansion is the program's own
//! business and nothing here proves what it produces.

/// Standard macros that leave the function instead of producing a value.
const MACRO_LEAVERS: [&str; 4] = ["panic", "unreachable", "todo", "unimplemented"];

/// Whether the macro written at a path leaves the function instead of
/// producing a value.
///
/// A position filled by one of these produces nothing at all, which the
/// language guarantees and which is a settled answer rather than a
/// refusal: an arm that panics is an arm that hands nothing back.
pub(in crate::semantic) fn macro_leaves(path: &syn::Path) -> bool {
    path.segments
        .last()
        .is_some_and(|segment| MACRO_LEAVERS.contains(&segment.ident.to_string().as_str()))
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use super::macro_leaves;

    fn path(written: &str) -> syn::Path {
        syn::parse_str(written).unwrap()
    }

    /// A macro that leaves the function is named here because leaving is
    /// a fact about control flow, not about a type — nothing a resolver
    /// states for the position answers it.
    #[test]
    fn a_macro_that_leaves_the_function_is_named() {
        assert!(macro_leaves(&path("panic")));
        assert!(macro_leaves(&path("std::unreachable")));
    }

    #[test]
    fn a_macro_of_the_program_itself_is_not_answered_for() {
        assert!(!macro_leaves(&path("my_own")));
        assert!(!macro_leaves(&path("tracing::info")));
    }
}
