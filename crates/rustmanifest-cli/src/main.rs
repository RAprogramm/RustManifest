// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Binary entry point — thin shim over [`rustmanifest_cli::run`].

use std::process::ExitCode;

use clap::Parser;
use rustmanifest_cli::{Cli, EXIT_ERROR};

fn main() -> ExitCode {
    let cli = Cli::parse();
    match rustmanifest_cli::run(cli) {
        Ok(code) => code,
        Err(err) => {
            tracing::error!(error = %err, "rustmanifest failed");
            ExitCode::from(EXIT_ERROR)
        }
    }
}
