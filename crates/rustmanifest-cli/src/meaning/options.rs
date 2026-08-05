// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Command surface of the `meaning` subcommand: the paths to analyze and
//! the rendering the caller asks for.

use std::path::PathBuf;

use clap::{Parser, ValueEnum};

/// Arguments for the `meaning` subcommand.
#[derive(Debug, Parser)]
pub struct MeaningArgs {
    /// Files or directories to analyze. Defaults to the current directory.
    #[arg(default_value = ".")]
    pub paths: Vec<PathBuf>,

    /// Output format.
    #[arg(long, value_enum, default_value_t = MeaningFormat::Text)]
    pub format: MeaningFormat,

    /// Resolution overlay written by `rustmanifest-resolve`, so a call
    /// into a library states the type it hands back instead of a refusal.
    #[arg(long)]
    pub resolution: Option<PathBuf>
}

/// Rendering of the meaning report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum MeaningFormat {
    /// Aligned table meant for a reader.
    Text,
    /// Canonical JSON array, one object per entry point.
    Json
}
