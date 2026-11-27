// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Command-line surface of the `coverage` subcommand.

use clap::Parser;

/// Arguments for the `coverage` subcommand.
#[derive(Debug, Parser)]
pub struct CoverageArgs {
    /// The program under analysis: the paths to read, and what a resolver
    /// settled about them. The measurement is read in the same mode the
    /// facts are, or it reports the coverage of a different run.
    #[command(flatten)]
    pub analyzed: crate::analyzed::Analyzed,

    /// List every unresolved `use` target with its occurrence count — the
    /// inspectable form of the report's "unresolved" bucket.
    #[arg(long)]
    pub unresolved: bool,

    /// What the run is handed beside the sources.
    #[command(flatten)]
    pub overlays: crate::overlays::Overlays,
}
