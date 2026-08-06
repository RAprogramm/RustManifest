// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Standard macros that build a value, and the type each one builds.
//!
//! A macro invocation is not an expression this tier can read into: what
//! it expands to is decided by the macro. For the few the standard
//! library defines, the type of the value is fixed whatever the arguments
//! are, and that is a fact worth stating — a position filled by one of
//! them is a value built there, not a shape the reading failed on.
//!
//! Only macros of the standard library are listed, matched on the last
//! segment of the written path. A macro the program defines itself is
//! absent, and stays a refusal: its expansion is the program's own
//! business and nothing here proves what it produces.

/// `(macro, built type)` rows for the standard macros that build a value.
const MACRO_VALUES: [(&str, &str); 9] = [
    ("format", "String"),
    ("vec", "Vec"),
    ("concat", "str"),
    ("stringify", "str"),
    ("matches", "bool"),
    ("write", "Result"),
    ("writeln", "Result"),
    ("format_args", "Arguments"),
    ("env", "str")
];

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

/// The type the macro written at a path builds, where the standard
/// library defines it.
pub(in crate::semantic) fn macro_value(path: &syn::Path) -> Option<&'static str> {
    let named = path.segments.last()?.ident.to_string();
    MACRO_VALUES
        .iter()
        .find(|(spelling, _)| *spelling == named)
        .map(|(_, built)| *built)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use super::macro_value;

    fn path(written: &str) -> syn::Path {
        syn::parse_str(written).unwrap()
    }

    #[test]
    fn a_listed_macro_names_what_it_builds() {
        assert_eq!(macro_value(&path("format")), Some("String"));
        assert_eq!(macro_value(&path("std::format")), Some("String"));
        assert_eq!(macro_value(&path("vec")), Some("Vec"));
    }

    #[test]
    fn a_macro_of_the_program_itself_is_not_answered_for() {
        assert_eq!(macro_value(&path("my_own")), None);
        assert_eq!(macro_value(&path("tracing::info")), None);
    }
}
