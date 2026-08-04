// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Owned source file representation used as the input unit for analyzers.
//!
//! Besides the path and text, a [`Source`] carries an engine-internal
//! pragma-usage recorder behind interior mutability. When an analyzer skips
//! an occurrence because an inline pragma suppressed it, the analyzer records
//! the pragma's line index on the source it was handed. The orchestrator,
//! which constructs one `Source` per file, reads the recorder after all
//! analyzers ran and emits an `RM-META-001` finding for every pragma that
//! suppressed nothing. This side channel keeps the public
//! `Analyzer::analyze(&self, source: &Source) -> Vec<Finding>` signature
//! unchanged.
//!
//! A source also carries the Rust syntax tree parsed from its text, behind
//! the same kind of lazy cell. Every tier above the pattern tier reads the
//! same tree of the same file, so parsing it per reader made one file parsed
//! dozens of times per run; parsing it on the source parses it once.

use std::{
    cell::OnceCell,
    path::PathBuf,
    sync::{Mutex, OnceLock}
};

/// A single source file under analysis.
///
/// `path` is the workspace-relative or absolute path used both for matching
/// against `exclude_globs` and for emitting [`Location`]s on findings. `text`
/// is the file contents loaded into memory.
///
/// The parsed tree is not part of the value's identity: a clone starts with
/// an empty cell and parses on demand, exactly like a freshly read file.
///
/// `syn` trees hold spans that are neither [`Send`] nor [`Sync`], so a
/// `Source` stays on the thread that built it. The engine's parallelism runs
/// across files, each with its own source, so nothing needs to cross.
///
/// [`Location`]: rustmanifest_schema::Location
pub struct Source {
    /// Path the findings of this file are located against.
    path:              PathBuf,
    /// Contents of the file, loaded into memory.
    text:              String,
    /// Syntax tree of [`Self::text`], or `None` when it is not Rust. Empty
    /// until the first reader asks.
    tree:              OnceCell<Option<syn::File>>,
    /// Lines of the pragmas that suppressed something during this run.
    used_pragma_lines: OnceLock<Mutex<Vec<usize>>>
}

impl std::fmt::Debug for Source {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Source")
            .field("path", &self.path)
            .field("text", &format_args!("{} bytes", self.text.len()))
            .field("tree", &self.tree.get().map(Option::is_some))
            .field("used_pragma_lines", &self.used_pragma_lines)
            .finish()
    }
}

impl Clone for Source {
    fn clone(&self) -> Self {
        let clone = Self::new(self.path.clone(), self.text.clone());
        for line_index in self.used_pragma_lines() {
            clone.record_pragma_use(line_index);
        }
        clone
    }
}

impl Source {
    /// Constructs a new source file representation.
    #[must_use]
    pub const fn new(path: PathBuf, text: String) -> Self {
        Self {
            path,
            text,
            tree: OnceCell::new(),
            used_pragma_lines: OnceLock::new()
        }
    }

    /// Returns the file path.
    #[must_use]
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// Renders the trailing three path components — enough to tell two
    /// same-named files in different crates apart without printing the
    /// whole path.
    #[must_use]
    pub fn short_path(&self) -> String {
        let mut components: Vec<String> = self
            .path
            .components()
            .rev()
            .take(3)
            .map(|component| component.as_os_str().to_string_lossy().into_owned())
            .collect();
        components.reverse();
        components.join("/")
    }

    /// Returns the file text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the file's text parsed as a Rust syntax tree, or `None` when
    /// it does not parse as Rust.
    ///
    /// The first call parses; every later call returns that same tree. A
    /// file the pattern tier alone visits is never parsed at all.
    #[must_use]
    pub fn tree(&self) -> Option<&syn::File> {
        self.tree
            .get_or_init(|| syn::parse_file(&self.text).ok())
            .as_ref()
    }

    /// Returns the size of the file in bytes.
    #[must_use]
    pub const fn len_bytes(&self) -> usize {
        self.text.len()
    }

    /// Records that the pragma on `line_index` suppressed at least one
    /// occurrence during this run.
    ///
    /// A poisoned recorder lock is ignored: losing a usage record can only
    /// cause a spurious unused-pragma warning, never a missed finding.
    pub(crate) fn record_pragma_use(&self, line_index: usize) {
        let recorder = self
            .used_pragma_lines
            .get_or_init(|| Mutex::new(Vec::new()));
        if let Ok(mut lines) = recorder.lock()
            && !lines.contains(&line_index)
        {
            lines.push(line_index);
        }
    }

    /// Returns the sorted line indices of pragmas that suppressed at least
    /// one occurrence during this run.
    pub(crate) fn used_pragma_lines(&self) -> Vec<usize> {
        let mut lines = self
            .used_pragma_lines
            .get()
            .and_then(|recorder| recorder.lock().ok())
            .map_or_else(Vec::new, |guard| guard.clone());
        lines.sort_unstable();
        lines
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

    fn source(text: &str) -> Source {
        Source::new(PathBuf::from("src/lib.rs"), text.to_owned())
    }

    #[test]
    fn tree_is_returned_for_rust_text() {
        assert_eq!(
            source("struct User { id: u64 }")
                .tree()
                .unwrap()
                .items
                .len(),
            1
        );
    }

    #[test]
    fn tree_is_none_for_text_that_is_not_rust() {
        assert!(source("this is not rust at all {{{").tree().is_none());
    }

    #[test]
    fn repeated_reads_share_one_tree() {
        let source = source("struct User { id: u64 }");
        let first = std::ptr::from_ref(source.tree().unwrap());
        let second = std::ptr::from_ref(source.tree().unwrap());
        assert_eq!(first, second);
    }

    #[test]
    fn nothing_is_parsed_before_the_first_read() {
        let source = source("struct User { id: u64 }");
        assert!(format!("{source:?}").contains("tree: None"));
        assert!(source.tree().is_some());
        assert!(format!("{source:?}").contains("tree: Some(true)"));
    }

    #[test]
    fn a_clone_starts_with_an_unparsed_tree() {
        let source = source("struct User { id: u64 }");
        assert!(source.tree().is_some());
        let copy = source.clone();
        assert!(format!("{copy:?}").contains("tree: None"));
        assert!(copy.tree().is_some());
        assert!(format!("{source:?}").contains("tree: Some(true)"));
    }

    #[test]
    fn records_pragma_use_sorted_and_deduplicated() {
        let source = Source::new(PathBuf::from("src/lib.rs"), "fn main() {}".to_owned());
        assert!(source.used_pragma_lines().is_empty());
        source.record_pragma_use(3);
        source.record_pragma_use(1);
        source.record_pragma_use(3);
        assert_eq!(source.used_pragma_lines(), vec![1, 3]);
    }

    #[test]
    fn clone_carries_recorded_pragma_use() {
        let source = Source::new(PathBuf::from("src/lib.rs"), "fn main() {}".to_owned());
        source.record_pragma_use(2);
        let copy = source.clone();
        source.record_pragma_use(5);
        assert_eq!(copy.used_pragma_lines(), vec![2]);
        assert_eq!(source.used_pragma_lines(), vec![2, 5]);
    }
}
