// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Error type shared across the evaluation harness.

use std::path::PathBuf;

use rustmanifest_engine::EngineError;
use thiserror::Error;

/// Unrecoverable operational failure inside the evaluation harness.
#[derive(Debug, Error)]
pub enum EvalError {
    /// Reading the corpus manifest failed.
    #[error("reading corpus manifest {path}: {source}")]
    CorpusRead {
        /// Manifest path that could not be read.
        path:   PathBuf,
        /// Underlying IO error.
        source: std::io::Error
    },

    /// Parsing the corpus manifest failed.
    #[error("parsing corpus manifest {path}: {source}")]
    CorpusParse {
        /// Manifest path that could not be parsed.
        path:   PathBuf,
        /// Underlying TOML deserialization error.
        source: Box<toml::de::Error>
    },

    /// A corpus entry points at a directory that does not exist.
    #[error("corpus checkout missing for repo {name} at {path}")]
    MissingCheckout {
        /// Corpus entry name.
        name: String,
        /// Expected checkout directory.
        path: PathBuf
    },

    /// Building or using a judgement bed failed.
    #[error("judgement bed for {name}: {source}")]
    Bed {
        /// Corpus entry name or bed path.
        name:   String,
        /// Underlying IO error.
        source: std::io::Error
    },

    /// The engine reported a hard failure.
    #[error("engine failure: {0}")]
    Engine(#[from] EngineError),

    /// The committed baseline is absent; `bless` has never been run.
    #[error("baseline missing at {path}; run `rustmanifest-eval bless` first")]
    MissingBaseline {
        /// Expected baseline path.
        path: PathBuf
    },

    /// Reading the committed baseline failed.
    #[error("reading baseline {path}: {source}")]
    BaselineRead {
        /// Baseline path that could not be read.
        path:   PathBuf,
        /// Underlying IO error.
        source: std::io::Error
    },

    /// Parsing the committed baseline failed.
    #[error("parsing baseline {path}: {source}")]
    BaselineParse {
        /// Baseline path that could not be parsed.
        path:   PathBuf,
        /// Underlying JSON deserialization error.
        source: serde_json::Error
    },

    /// Serializing a report to JSON failed.
    #[error("serializing report: {0}")]
    Serialize(#[from] serde_json::Error),

    /// Writing an output file failed.
    #[error("writing {path}: {source}")]
    WriteFile {
        /// Output path that could not be written.
        path:   PathBuf,
        /// Underlying IO error.
        source: std::io::Error
    },

    /// Writing the human summary to stdout failed.
    #[error("writing summary: {0}")]
    Stdout(#[from] std::io::Error)
}
