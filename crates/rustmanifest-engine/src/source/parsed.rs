// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! One file under analysis, holding the syntax tree parsed from it.
//!
//! Every analyzer above the pattern tier needs the same tree of the same
//! file. Handing each of them the raw [`Source`] made each of them parse it,
//! so a file crossed by eight structural rules was parsed eight times. This
//! module carries the tree beside the source instead, parsed once and read
//! by all of them.

use std::cell::OnceCell;

use crate::source::Source;

/// A [`Source`] together with its Rust syntax tree, parsed on first demand.
///
/// Parsing stays lazy, so a run of pattern-tier rules alone never pays for
/// a tree nobody reads, and a file that does not parse as Rust records that
/// verdict once rather than rediscovering it per analyzer.
///
/// `syn` trees carry spans that are neither [`Send`] nor [`Sync`], so a
/// `ParsedSource` never leaves the thread that built it — the engine builds
/// one inside each per-file task. The [`Source`] it borrows carries no such
/// restriction and stays shareable across the worker pool.
pub struct ParsedSource<'source> {
    /// The file this unit speaks for.
    source: &'source Source,
    /// The tree parsed from [`Self::source`], or `None` when its text is not
    /// Rust. Empty until the first reader asks.
    tree:   OnceCell<Option<syn::File>>
}

impl std::fmt::Debug for ParsedSource<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ParsedSource")
            .field("source", &self.source)
            .field("parsed", &self.tree.get().map(Option::is_some))
            .finish()
    }
}

impl<'source> ParsedSource<'source> {
    /// Wraps `source`. Nothing is parsed until [`ParsedSource::tree`] is
    /// called.
    #[must_use]
    pub const fn new(source: &'source Source) -> Self {
        Self {
            source,
            tree: OnceCell::new()
        }
    }

    /// Returns the source this unit was built from.
    #[must_use]
    pub const fn source(&self) -> &'source Source {
        self.source
    }

    /// Returns the file parsed as Rust, or `None` when its text does not
    /// parse.
    ///
    /// The first call parses the text; every later call returns that same
    /// tree without parsing again.
    #[must_use]
    pub fn tree(&self) -> Option<&syn::File> {
        self.tree
            .get_or_init(|| syn::parse_file(self.source.text()).ok())
            .as_ref()
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
    use std::path::PathBuf;

    use super::*;

    fn source(text: &str) -> Source {
        Source::new(PathBuf::from("src/lib.rs"), text.to_owned())
    }

    #[test]
    fn tree_is_returned_for_rust_text() {
        let source = source("struct User { id: u64 }");
        let parsed = ParsedSource::new(&source);
        assert_eq!(parsed.tree().unwrap().items.len(), 1);
    }

    #[test]
    fn tree_is_none_for_text_that_is_not_rust() {
        let source = source("this is not rust at all {{{");
        let parsed = ParsedSource::new(&source);
        assert!(parsed.tree().is_none());
    }

    #[test]
    fn repeated_reads_share_one_tree() {
        let source = source("struct User { id: u64 }");
        let parsed = ParsedSource::new(&source);
        let first = std::ptr::from_ref(parsed.tree().unwrap());
        let second = std::ptr::from_ref(parsed.tree().unwrap());
        assert_eq!(first, second);
    }

    #[test]
    fn nothing_is_parsed_before_the_first_read() {
        let source = source("struct User { id: u64 }");
        let parsed = ParsedSource::new(&source);
        assert!(format!("{parsed:?}").contains("parsed: None"));
        assert!(parsed.tree().is_some());
        assert!(format!("{parsed:?}").contains("parsed: Some(true)"));
    }

    #[test]
    fn the_borrowed_source_is_handed_back() {
        let source = source("fn main() {}");
        let parsed = ParsedSource::new(&source);
        assert_eq!(parsed.source().text(), "fn main() {}");
    }
}
