// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Tier-2 AST analyzer: built-in `syn`-based structural checks.
//!
//! Each rule of the `ast` tier names one built-in check. The analyzer parses
//! a source file once, hands the tree to the check, and converts the
//! occurrences it reports into findings, honoring inline pragma suppression
//! exactly like the pattern tier.
//!
//! This root carries the vocabulary every check speaks: the [`Occurrence`] a
//! check reports and the [`flattened`] view of a parsed file's items.

mod analyzer;
mod api_size;
mod check;
mod constant_encapsulation;
mod constructor_design;
mod delegating_constructors;
mod entity_naming;
mod fakes_over_mocks;
mod immutability_first;
mod method_naming;

use rustmanifest_schema::Range;

pub use self::analyzer::AstAnalyzer;

/// One structural violation reported by a built-in check.
pub(crate) struct Occurrence {
    /// Byte range of the offending item.
    range:  Range,
    /// Check-specific description of the violation.
    detail: String
}

impl Occurrence {
    /// Creates an occurrence for the item spanned by `span`.
    pub(crate) fn new(span: &proc_macro2::Span, detail: String) -> Self {
        let bytes = span.byte_range();
        Self {
            range: Range {
                start: u32::try_from(bytes.start).unwrap_or(u32::MAX),
                end:   u32::try_from(bytes.end).unwrap_or(u32::MAX)
            },
            detail
        }
    }
}

/// Appends `items` and the items of every nested inline module to `out`.
pub(crate) fn flattened<'a>(items: &'a [syn::Item], out: &mut Vec<&'a syn::Item>) {
    for item in items {
        out.push(item);
        if let syn::Item::Mod(module) = item
            && let Some((_, nested)) = &module.content
        {
            flattened(nested, out);
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use super::*;

    #[test]
    fn nested_module_items_are_flattened() {
        let file = syn::parse_file("mod outer { mod inner { struct DataManager; } }").unwrap();
        let mut items = Vec::new();
        flattened(&file.items, &mut items);
        assert_eq!(items.len(), 3);
    }
}
