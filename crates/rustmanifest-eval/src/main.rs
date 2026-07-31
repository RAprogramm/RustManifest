// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Binary entry point — thin shim over [`rustmanifest_eval::run`].
#![allow(
    clippy::multiple_crate_versions,
    reason = "duplicate transitive versions are pulled in by upstream dependencies"
)]

use std::process::ExitCode;

use clap::Parser;
use rustmanifest_eval::{Cli, EXIT_ERROR};

fn main() -> ExitCode {
    let cli = Cli::parse();
    match rustmanifest_eval::run(cli) {
        Ok(code) => code,
        Err(err) => {
            tracing::error!(error = %err, "rustmanifest-eval failed");
            ExitCode::from(EXIT_ERROR)
        }
    }
}
