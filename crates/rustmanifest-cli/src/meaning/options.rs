// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Command surface of the `meaning` subcommand: the paths to analyze and
//! the rendering the caller asks for.

use clap::{Parser, ValueEnum};

/// Arguments for the `meaning` subcommand.
#[derive(Debug, Parser)]
pub struct MeaningArgs {
    /// The program under analysis.
    #[command(flatten)]
    pub analyzed: crate::analyzed::Analyzed,

    /// Output format.
    #[arg(long, value_enum, default_value_t = MeaningFormat::Text)]
    pub format: MeaningFormat
}

/// Rendering of the meaning report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum MeaningFormat {
    /// Aligned table meant for a reader.
    Text,
    /// Canonical JSON array, one object per entry point.
    Json
}
