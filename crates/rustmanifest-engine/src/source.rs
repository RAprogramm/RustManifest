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

use std::{
    path::PathBuf,
    sync::{Mutex, OnceLock}
};

/// A single source file under analysis.
///
/// `path` is the workspace-relative or absolute path used both for matching
/// against `exclude_globs` and for emitting [`Location`]s on findings. `text`
/// is the file contents loaded into memory.
///
/// [`Location`]: rustmanifest_schema::Location
#[derive(Debug)]
pub struct Source {
    path:              PathBuf,
    text:              String,
    used_pragma_lines: OnceLock<Mutex<Vec<usize>>>
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
