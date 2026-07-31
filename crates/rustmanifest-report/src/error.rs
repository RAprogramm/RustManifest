// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Error type shared by the three renderers.

use thiserror::Error;

/// Errors produced by renderers.
#[derive(Debug, Error)]
pub enum ReportError {
    /// IO error while writing to the destination.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error from `serde_json`.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error)
}
