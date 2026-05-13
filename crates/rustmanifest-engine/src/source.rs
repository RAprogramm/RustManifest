// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Owned source file representation used as the input unit for analyzers.

use std::path::PathBuf;

/// A single source file under analysis.
///
/// `path` is the workspace-relative or absolute path used both for matching
/// against `exclude_globs` and for emitting [`Location`]s on findings. `text`
/// is the file contents loaded into memory.
///
/// [`Location`]: rustmanifest_schema::Location
#[derive(Debug, Clone)]
pub struct Source {
    path: PathBuf,
    text: String
}

impl Source {
    /// Constructs a new source file representation.
    #[must_use]
    pub const fn new(path: PathBuf, text: String) -> Self {
        Self {
            path,
            text
        }
    }

    /// Returns the file path.
    #[must_use]
    pub fn path(&self) -> &std::path::Path {
        &self.path
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
}
