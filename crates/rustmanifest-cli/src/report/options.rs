// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Command surface of the `report` subcommand: the paths to analyze and
//! the cap on every ranked list it prints.

use std::path::PathBuf;

use clap::Parser;

/// Arguments for the `report` subcommand.
#[derive(Debug, Parser)]
pub struct ReportArgs {
    /// Files or directories to analyze. Defaults to the current directory.
    #[arg(default_value = ".")]
    pub paths: Vec<PathBuf>,

    /// Cap for every ranked list in the report.
    #[arg(long, default_value_t = 10)]
    pub top: usize,

    /// Resolution overlay written by `rustmanifest-resolve`, so a call
    /// into a library states the type it hands back instead of a refusal.
    #[arg(long)]
    pub resolution: Option<PathBuf>
}
