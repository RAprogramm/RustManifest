// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Error types for the analysis engine.

use std::path::PathBuf;

use masterror::AppError;

/// Errors produced by the engine during analysis.
#[derive(Debug)]
pub enum EngineError {
    /// Failed to read a source file.
    Read {
        /// File path that could not be read.
        path:   PathBuf,
        /// Underlying IO error.
        source: std::io::Error
    },

    /// A rule's regex failed to compile.
    InvalidRegex {
        /// Rule id whose regex did not compile.
        rule_id: String,
        /// Raw regex string.
        regex:   String,
        /// Underlying regex error.
        source:  regex::Error
    },

    /// A glob pattern in the rule's `exclude_globs` failed to compile.
    InvalidGlob {
        /// Rule id whose glob did not compile.
        rule_id: String,
        /// Raw glob pattern.
        glob:    String,
        /// Underlying glob error.
        source:  globset::Error
    },

    /// An `ast`-tier rule names a check this engine build does not provide.
    UnknownAstCheck {
        /// Rule id naming the unknown check.
        rule_id: String,
        /// Check identifier that failed to resolve.
        check:   String
    },

    /// The file walker reported an error visiting an entry.
    Walker(ignore::Error),

    /// Analysis was cancelled by the caller.
    Cancelled
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read {
                path,
                source
            } => {
                write!(f, "failed to read {}: {source}", path.display())
            }
            Self::InvalidRegex {
                rule_id,
                regex,
                source
            } => write!(f, "rule {rule_id}: invalid regex {regex:?}: {source}"),
            Self::InvalidGlob {
                rule_id,
                glob,
                source
            } => write!(f, "rule {rule_id}: invalid glob {glob:?}: {source}"),
            Self::UnknownAstCheck {
                rule_id,
                check
            } => {
                write!(f, "rule {rule_id}: unknown ast check {check:?}")
            }
            Self::Walker(error) => write!(f, "walker error: {error}"),
            Self::Cancelled => f.write_str("analysis cancelled")
        }
    }
}

impl std::error::Error for EngineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read {
                source, ..
            } => Some(source),
            Self::InvalidRegex {
                source, ..
            } => Some(source),
            Self::InvalidGlob {
                source, ..
            } => Some(source),
            Self::Walker(error) => Some(error),
            Self::UnknownAstCheck {
                ..
            }
            | Self::Cancelled => None
        }
    }
}

impl From<ignore::Error> for EngineError {
    fn from(error: ignore::Error) -> Self {
        Self::Walker(error)
    }
}

impl From<EngineError> for AppError {
    fn from(error: EngineError) -> Self {
        let message = error.to_string();
        match &error {
            EngineError::Read {
                ..
            }
            | EngineError::Walker(_)
            | EngineError::Cancelled => Self::internal(message).with_context(error),
            EngineError::InvalidRegex {
                ..
            }
            | EngineError::InvalidGlob {
                ..
            }
            | EngineError::UnknownAstCheck {
                ..
            } => Self::bad_request(message).with_context(error)
        }
    }
}

/// Convenience result alias for engine fallible operations.
pub type EngineResult<T> = Result<T, AppError>;
