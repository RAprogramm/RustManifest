// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Command-line frontend for the `rustmanifest` engine.
//!
//! Phase 0 exposes only `--version` and the subcommand surface so that the
//! shape of the CLI is locked while the engine is still being built.

use clap::{Parser, Subcommand};

/// Top-level CLI arguments.
#[derive(Debug, Parser)]
#[command(
    name = "rustmanifest",
    version,
    about = "Production-grade Rust review engine — methodology-as-code"
)]
struct Cli {
    /// Selected subcommand.
    #[command(subcommand)]
    command: Command
}

/// Available subcommands for the `rustmanifest` binary.
#[derive(Debug, Subcommand)]
enum Command {
    /// Print the build metadata and exit.
    Version
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::Version => {}
    }
}
