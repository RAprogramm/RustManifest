// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Error type shared by the three renderers.

use masterror::AppError;

/// Errors produced by renderers.
#[derive(Debug)]
pub enum ReportError {
    /// IO error while writing to the destination.
    Io(std::io::Error),

    /// Serialization error from `serde_json`.
    Serialization(serde_json::Error)
}

impl std::fmt::Display for ReportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "io error: {error}"),
            Self::Serialization(error) => write!(f, "serialization error: {error}")
        }
    }
}

impl std::error::Error for ReportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Serialization(error) => Some(error)
        }
    }
}

impl From<std::io::Error> for ReportError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for ReportError {
    fn from(error: serde_json::Error) -> Self {
        Self::Serialization(error)
    }
}

impl From<ReportError> for AppError {
    fn from(error: ReportError) -> Self {
        let message = error.to_string();
        match error {
            ReportError::Io(source) => Self::internal(message).with_context(source),
            ReportError::Serialization(source) => Self::internal(message).with_context(source)
        }
    }
}
