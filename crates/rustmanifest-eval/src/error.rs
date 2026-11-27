// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Error type shared across the evaluation harness.

use std::path::PathBuf;

use masterror::AppError;
use rustmanifest_engine::EngineError;

/// Unrecoverable operational failure inside the evaluation harness.
#[derive(Debug)]
pub enum EvalError {
    /// Reading the corpus manifest failed.
    CorpusRead {
        /// Manifest path that could not be read.
        path:   PathBuf,
        /// Underlying IO error.
        source: std::io::Error
    },

    /// Parsing the corpus manifest failed.
    CorpusParse {
        /// Manifest path that could not be parsed.
        path:   PathBuf,
        /// Underlying TOML deserialization error.
        source: Box<toml::de::Error>
    },

    /// A corpus entry points at a directory that does not exist.
    MissingCheckout {
        /// Corpus entry name.
        name: String,
        /// Expected checkout directory.
        path: PathBuf
    },

    /// Building or using a judgement bed failed.
    Bed {
        /// Corpus entry name or bed path.
        name:   String,
        /// Underlying IO error.
        source: std::io::Error
    },

    /// The engine reported a hard failure.
    Engine(EngineError),

    /// The committed baseline is absent; `bless` has never been run.
    MissingBaseline {
        /// Expected baseline path.
        path: PathBuf
    },

    /// Reading the committed baseline failed.
    BaselineRead {
        /// Baseline path that could not be read.
        path:   PathBuf,
        /// Underlying IO error.
        source: std::io::Error
    },

    /// Parsing the committed baseline failed.
    BaselineParse {
        /// Baseline path that could not be parsed.
        path:   PathBuf,
        /// Underlying JSON deserialization error.
        source: serde_json::Error
    },

    /// Serializing a report to JSON failed.
    Serialize(serde_json::Error),

    /// Writing an output file failed.
    WriteFile {
        /// Path the output could not be written to.
        path:   PathBuf,
        /// Underlying IO error.
        source: std::io::Error
    },

    /// Writing the human summary to stdout failed.
    Stdout(std::io::Error)
}

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CorpusRead {
                path,
                source
            } => {
                write!(f, "reading corpus manifest {}: {source}", path.display())
            }
            Self::CorpusParse {
                path,
                source
            } => {
                write!(f, "parsing corpus manifest {}: {source}", path.display())
            }
            Self::MissingCheckout {
                name,
                path
            } => {
                write!(
                    f,
                    "corpus checkout missing for repo {name} at {}",
                    path.display()
                )
            }
            Self::Bed {
                name,
                source
            } => write!(f, "judgement bed for {name}: {source}"),
            Self::Engine(error) => write!(f, "engine failure: {error}"),
            Self::MissingBaseline {
                path
            } => write!(
                f,
                "baseline missing at {}; run `rustmanifest-eval bless` first",
                path.display()
            ),
            Self::BaselineRead {
                path,
                source
            } => {
                write!(f, "reading baseline {}: {source}", path.display())
            }
            Self::BaselineParse {
                path,
                source
            } => {
                write!(f, "parsing baseline {}: {source}", path.display())
            }
            Self::Serialize(error) => write!(f, "serializing report: {error}"),
            Self::WriteFile {
                path,
                source
            } => write!(f, "writing {}: {source}", path.display()),
            Self::Stdout(error) => write!(f, "writing summary: {error}")
        }
    }
}

impl std::error::Error for EvalError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CorpusRead {
                source, ..
            }
            | Self::Bed {
                source, ..
            }
            | Self::BaselineRead {
                source, ..
            }
            | Self::WriteFile {
                source, ..
            }
            | Self::Stdout(source) => Some(source),
            Self::CorpusParse {
                source, ..
            } => Some(source.as_ref()),
            Self::Engine(error) => Some(error),
            Self::BaselineParse {
                source, ..
            }
            | Self::Serialize(source) => Some(source),
            Self::MissingCheckout {
                ..
            }
            | Self::MissingBaseline {
                ..
            } => None
        }
    }
}

impl From<EngineError> for EvalError {
    fn from(error: EngineError) -> Self {
        Self::Engine(error)
    }
}

impl From<serde_json::Error> for EvalError {
    fn from(error: serde_json::Error) -> Self {
        Self::Serialize(error)
    }
}

impl From<std::io::Error> for EvalError {
    fn from(error: std::io::Error) -> Self {
        Self::Stdout(error)
    }
}

impl From<EvalError> for AppError {
    fn from(error: EvalError) -> Self {
        let message = error.to_string();
        match error {
            EvalError::Engine(source) => Self::internal(message).with_context(source),
            EvalError::CorpusRead {
                source, ..
            }
            | EvalError::Bed {
                source, ..
            }
            | EvalError::BaselineRead {
                source, ..
            }
            | EvalError::WriteFile {
                source, ..
            }
            | EvalError::Stdout(source) => Self::internal(message).with_context(source),
            EvalError::CorpusParse {
                source, ..
            } => Self::internal(message).with_context(source),
            EvalError::BaselineParse {
                source, ..
            }
            | EvalError::Serialize(source) => Self::internal(message).with_context(source),
            EvalError::MissingCheckout {
                ..
            }
            | EvalError::MissingBaseline {
                ..
            } => Self::not_found(message)
        }
    }
}

impl From<AppError> for EvalError {
    fn from(error: AppError) -> Self {
        Self::Stdout(std::io::Error::other(error.to_string()))
    }
}
