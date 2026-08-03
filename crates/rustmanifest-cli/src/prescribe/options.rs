// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Command-line surface of the `prescribe` subcommand.

use std::path::PathBuf;

use clap::Parser;

/// Arguments for the `prescribe` subcommand.
#[derive(Debug, Parser)]
pub struct PrescribeArgs {
    /// Files or directories to analyze. Defaults to the current directory.
    #[arg(default_value = ".")]
    pub paths: Vec<PathBuf>,

    /// Print the full derivation behind every prescription: the counts
    /// before and after, and the facts each count rests on.
    #[arg(long)]
    pub why: bool
}
