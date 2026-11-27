// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What can go wrong while resolving a workspace.
//!
//! Loading is the only failing step: once a workspace is loaded, an
//! unanswerable position is silence in the overlay rather than an error,
//! because "the resolver did not settle this" is a fact about that position
//! and not a failure of the run.

use std::path::PathBuf;

use masterror::AppError;

/// Why a workspace could not be resolved.
#[derive(Debug)]
pub enum ResolveError {
    /// The cargo workspace at this path could not be loaded.
    Load {
        /// Path the load was attempted at.
        path:   PathBuf,
        /// What the loader reported.
        reason: String
    },
    /// The overlay could not be written or read as JSON.
    Overlay(serde_json::Error),
    /// An overlay file could not be read or written.
    File {
        /// Path the operation was attempted on.
        path:   PathBuf,
        /// The underlying failure.
        source: std::io::Error
    }
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Load {
                path,
                reason
            } => {
                write!(
                    f,
                    "loading the workspace at {} failed: {reason}",
                    path.display()
                )
            }
            Self::Overlay(_) => f.write_str("the resolution overlay could not be carried as JSON"),
            Self::File {
                path, ..
            } => {
                write!(f, "reading or writing {} failed", path.display())
            }
        }
    }
}

impl std::error::Error for ResolveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Overlay(error) => Some(error),
            Self::File {
                source, ..
            } => Some(source),
            Self::Load {
                ..
            } => None
        }
    }
}

impl From<serde_json::Error> for ResolveError {
    fn from(error: serde_json::Error) -> Self {
        Self::Overlay(error)
    }
}

impl From<ResolveError> for AppError {
    fn from(error: ResolveError) -> Self {
        let message = error.to_string();
        match error {
            ResolveError::Load {
                ..
            } => Self::internal(message),
            ResolveError::Overlay(source) => Self::internal(message).with_context(source),
            ResolveError::File {
                source, ..
            } => Self::internal(message).with_context(source)
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
    fn a_load_failure_names_the_path_and_the_reason() {
        let error = ResolveError::Load {
            path:   PathBuf::from("/nowhere"),
            reason: "no manifest".to_owned()
        };
        let rendered = error.to_string();
        assert!(rendered.contains("/nowhere"));
        assert!(rendered.contains("no manifest"));
    }

    #[test]
    fn a_file_failure_names_the_path() {
        let error = ResolveError::File {
            path:   PathBuf::from("overlay.json"),
            source: std::io::Error::other("denied")
        };
        assert!(error.to_string().contains("overlay.json"));
    }
}
