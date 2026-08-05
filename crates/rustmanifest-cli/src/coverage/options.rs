// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Command-line surface of the `coverage` subcommand.

use std::path::PathBuf;

use clap::Parser;

/// Arguments for the `coverage` subcommand.
#[derive(Debug, Parser)]
pub struct CoverageArgs {
    /// Files or directories to analyze. Defaults to the current directory.
    #[arg(default_value = ".")]
    pub paths: Vec<PathBuf>,

    /// List every unresolved `use` target with its occurrence count — the
    /// inspectable form of the report's "unresolved" bucket.
    #[arg(long)]
    pub unresolved: bool,

    /// What the run is handed beside the sources.
    #[command(flatten)]
    pub overlays: crate::overlays::Overlays
}
