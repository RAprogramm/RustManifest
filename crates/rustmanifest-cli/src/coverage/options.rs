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

    /// Also expand every crate via `cargo +nightly rustc -- \
    /// -Zunpretty=expanded` and report what derives generate.
    #[arg(long)]
    pub expand: bool,

    /// List every unresolved `use` target with its occurrence count — the
    /// inspectable form of the report's "unresolved" bucket.
    #[arg(long)]
    pub unresolved: bool,

    /// SCIP index file (`rust-analyzer scip .`) overlaying name-resolved
    /// call edges; the report counts what the index added separately.
    #[arg(long, value_name = "INDEX")]
    pub scip: Option<PathBuf>
}
