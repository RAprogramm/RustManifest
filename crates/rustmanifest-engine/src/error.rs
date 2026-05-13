// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Error types for the analysis engine.

use std::path::PathBuf;

use thiserror::Error;

/// Errors produced by the engine during analysis.
#[derive(Debug, Error)]
pub enum EngineError {
    /// Failed to read a source file.
    #[error("failed to read {path}: {source}")]
    Read {
        /// File path that could not be read.
        path:   PathBuf,
        /// Underlying IO error.
        #[source]
        source: std::io::Error
    },

    /// A rule's regex failed to compile.
    #[error("rule {rule_id}: invalid regex {regex:?}: {source}")]
    InvalidRegex {
        /// Rule id whose regex did not compile.
        rule_id: String,
        /// Raw regex string.
        regex:   String,
        /// Underlying regex error.
        #[source]
        source:  regex::Error
    },

    /// A glob pattern in the rule's `exclude_globs` failed to compile.
    #[error("rule {rule_id}: invalid glob {glob:?}: {source}")]
    InvalidGlob {
        /// Rule id whose glob did not compile.
        rule_id: String,
        /// Raw glob pattern.
        glob:    String,
        /// Underlying glob error.
        #[source]
        source:  globset::Error
    },

    /// The file walker reported an error visiting an entry.
    #[error("walker error: {0}")]
    Walker(#[from] ignore::Error),

    /// Analysis was cancelled by the caller.
    #[error("analysis cancelled")]
    Cancelled
}

/// Convenience result alias for engine fallible operations.
pub type EngineResult<T> = Result<T, EngineError>;
