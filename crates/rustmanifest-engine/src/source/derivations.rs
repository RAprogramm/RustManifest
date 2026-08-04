// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a file's own text determines, derived at most once per file.
//!
//! A syntax tree and a list of inline pragmas are both functions of the text
//! alone: same text, same answer, however many readers ask. Every tier asks
//! for both of the same file, so deriving them at each reader had one file
//! parsed and rescanned dozens of times per run.
//!
//! They live in their own value rather than as loose fields of a
//! [`Source`](super::Source) because they are one concern — the text read as
//! something other than bytes — kept apart from the file's identity and from
//! the per-run suppression record.

use std::cell::OnceCell;

use crate::pragma::{self, Pragma};

/// The syntax tree and inline pragmas of one file, each derived on first
/// demand and reused afterwards.
///
/// Derivation is lazy in both directions: a run of pattern-tier rules never
/// parses, and a file no pragma-aware tier visits is never scanned.
#[derive(Default)]
pub(super) struct Derivations {
    /// Syntax tree of the text, or `None` when the text is not Rust.
    tree:    OnceCell<Option<syn::File>>,
    /// Inline suppression pragmas written in the text.
    pragmas: OnceCell<Vec<Pragma>>
}

impl std::fmt::Debug for Derivations {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Derivations")
            .field("tree", &self.tree.get().map(Option::is_some))
            .field("pragmas", &self.pragmas.get().map(Vec::len))
            .finish()
    }
}

impl Derivations {
    /// Creates an empty set; nothing is derived until a reader asks.
    pub(super) const fn new() -> Self {
        Self {
            tree:    OnceCell::new(),
            pragmas: OnceCell::new()
        }
    }

    /// Returns `text` parsed as a Rust syntax tree, or `None` when it does
    /// not parse. The first call parses; later calls reuse that tree.
    pub(super) fn tree(&self, text: &str) -> Option<&syn::File> {
        self.tree
            .get_or_init(|| syn::parse_file(text).ok())
            .as_ref()
    }

    /// Returns the inline pragmas written in `text`. The first call scans;
    /// later calls reuse that list.
    pub(super) fn pragmas(&self, text: &str) -> &[Pragma] {
        self.pragmas.get_or_init(|| pragma::collect(text))
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
    fn nothing_is_derived_before_the_first_read() {
        let rendered = format!("{:?}", Derivations::new());
        assert!(rendered.contains("tree: None"));
        assert!(rendered.contains("pragmas: None"));
    }

    #[test]
    fn a_tree_is_parsed_once_and_reused() {
        let derivations = Derivations::new();
        let text = "struct User { id: u64 }";
        let first = std::ptr::from_ref(derivations.tree(text).unwrap());
        let second = std::ptr::from_ref(derivations.tree(text).unwrap());
        assert_eq!(first, second);
    }

    #[test]
    fn text_that_is_not_rust_has_no_tree() {
        assert!(Derivations::new().tree("this is not rust {{{").is_none());
    }

    #[test]
    fn pragmas_are_collected_once_and_reused() {
        let derivations = Derivations::new();
        let text = "// rustmanifest: allow(RM-SEC-001) reason=\"fixture\"\nfn main() {}\n";
        assert_eq!(derivations.pragmas(text).len(), 1);
        assert_eq!(derivations.pragmas(text)[0].rule_id, "RM-SEC-001");
        assert_eq!(
            derivations.pragmas(text).as_ptr(),
            derivations.pragmas(text).as_ptr()
        );
    }

    #[test]
    fn reading_one_derivation_leaves_the_other_underived() {
        let derivations = Derivations::new();
        assert!(derivations.tree("fn main() {}").is_some());
        assert!(format!("{derivations:?}").contains("pragmas: None"));
    }
}
