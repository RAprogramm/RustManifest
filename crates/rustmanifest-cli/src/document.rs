// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Producing the compiler's own documentation index for a workspace.
//!
//! The trait oracle reads whatever emissions it finds under the target
//! directory and answers nothing where none exist. Nothing in this project
//! ever produced one, so the oracle was populated only when a developer had
//! happened to run the emitter by hand — which is to say almost never, and
//! the tier below it filled the silence with hand-written tables.
//!
//! This is the producer. It asks the compiler for the index the same way
//! the check run asks the linter for its diagnostics: as a subprocess, with
//! every failure degrading to a warning rather than to a wrong answer.
//!
//! The emitter is nightly-only, and the toolchain this workspace pins is
//! not. The command therefore names the nightly toolchain explicitly; where
//! none is installed, the run says so and writes nothing, because a missing
//! index is an answered question — the oracle reports absence — while a
//! half-written one is not.

use std::{io::Write as _, path::PathBuf, process::ExitCode};

use clap::Args;
use masterror::ResultExt as _;
use tracing::{info, warn};

use crate::{EXIT_CLEAN, EXIT_ERROR};

/// Arguments of the `document` subcommand.
#[derive(Debug, Args)]
pub struct DocumentArgs {
    /// Workspace root to document. Defaults to the current directory.
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Toolchain to ask for the index. The emitter is unstable, so this
    /// must be a nightly one.
    #[arg(long, default_value = "nightly")]
    pub toolchain: String
}

/// Emits the documentation index of the workspace and reports what landed.
///
/// # Errors
///
/// Returns an [`masterror::AppError`] only where the workspace root cannot be
/// read at all; a missing toolchain or a failing emitter is reported and
/// exits with the operational code.
pub fn run_document(args: &DocumentArgs) -> masterror::AppResult<ExitCode> {
    if !args.path.join("Cargo.toml").is_file() {
        warn!(path = %args.path.display(), "no Cargo.toml in the path to document");
        return Ok(ExitCode::from(EXIT_ERROR));
    }
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    match emit(args) {
        Ok(()) => {
            let written = emissions(&args.path);
            info!(count = written.len(), "documentation index written");
            writeln!(
                handle,
                "documentation index: {} emission(s) under {}",
                written.len(),
                index_dir(&args.path).display()
            )
            .context("writing the documentation report")?;
            Ok(ExitCode::from(EXIT_CLEAN))
        }
        Err(reason) => {
            warn!(%reason, "the documentation index was not produced");
            writeln!(handle, "documentation index not produced: {reason}")
                .context("writing the documentation report")?;
            Ok(ExitCode::from(EXIT_ERROR))
        }
    }
}

/// Runs the emitter over every workspace member.
///
/// The flags reach the emitter through the environment rather than after a
/// separator: the per-target command takes trailing flags but documents one
/// target, and the workspace-wide one takes no trailing flags at all.
/// Dependencies are left undocumented — their emissions would be the bulk
/// of the run and the oracle asks only about types this workspace declares.
fn emit(args: &DocumentArgs) -> Result<(), String> {
    let output = std::process::Command::new("cargo")
        .arg(format!("+{}", args.toolchain))
        .args(["doc", "--workspace", "--no-deps", "--quiet"])
        .current_dir(&args.path)
        .env("CARGO_TERM_COLOR", "never")
        .env(
            "RUSTDOCFLAGS",
            "-Zunstable-options --output-format json --document-private-items"
        )
        .output()
        .map_err(|error| format!("the emitter could not be invoked: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "the emitter exited with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
            .lines()
            .next_back()
            .unwrap_or("no message")
    ))
}

/// Where the emitter writes its index.
fn index_dir(root: &std::path::Path) -> PathBuf {
    root.join("target").join("doc")
}

/// Every emission the index directory holds.
fn emissions(root: &std::path::Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(index_dir(root)) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|held| held == "json"))
        .collect()
}
