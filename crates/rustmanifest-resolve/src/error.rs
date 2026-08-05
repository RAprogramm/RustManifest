// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What can go wrong while resolving a workspace.
//!
//! Loading is the only failing step: once a workspace is loaded, an
//! unanswerable position is silence in the overlay rather than an error,
//! because "the resolver did not settle this" is a fact about that position
//! and not a failure of the run.

use std::path::PathBuf;

use thiserror::Error;

/// Why a workspace could not be resolved.
#[derive(Debug, Error)]
pub enum ResolveError {
    /// The cargo workspace at this path could not be loaded.
    #[error("loading the workspace at {path} failed: {reason}")]
    Load {
        /// Path the load was attempted at.
        path:   PathBuf,
        /// What the loader reported.
        reason: String
    },
    /// The overlay could not be written or read as JSON.
    #[error("the resolution overlay could not be carried as JSON")]
    Overlay(#[from] serde_json::Error),
    /// An overlay file could not be read or written.
    #[error("reading or writing {path} failed")]
    File {
        /// Path the operation was attempted on.
        path:   PathBuf,
        /// The underlying failure.
        #[source]
        source: std::io::Error
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
