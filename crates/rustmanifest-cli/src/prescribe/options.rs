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
    pub why: bool,

    /// Resolution overlay written by `rustmanifest-resolve`, so a
    /// canonical body rests on the traits the compiler says a field's
    /// type carries rather than on nothing at all.
    #[arg(long)]
    pub resolution: Option<PathBuf>,
}
