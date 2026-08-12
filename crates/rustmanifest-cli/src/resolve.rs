// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Producing the resolution a workspace is read against.
//!
//! Every tier that answers about types, modules, calls or signatures reads
//! this overlay, and every one of them is silent without it — deliberately,
//! because the alternative is a guess that reads like an answer. Until now
//! producing it meant knowing that a second binary exists and running it by
//! hand, which is a thing nobody knows on the first day.
//!
//! The resolver lives in its own binary so the language-server libraries
//! stay out of the tool a person runs on every save. This subcommand is the
//! bridge: it runs that binary the way the check run runs the linter, and
//! every failure degrades to a report rather than to a wrong answer.

use std::{io::Write as _, path::PathBuf, process::ExitCode};

use anyhow::Context as _;
use clap::Args;
use tracing::{info, warn};

use crate::{EXIT_CLEAN, EXIT_ERROR};

/// Arguments of the `resolve` subcommand.
#[derive(Debug, Args)]
pub struct ResolveArgs {
    /// Workspace root to resolve. Defaults to the current directory.
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Where to write the resolution. Defaults to `resolution.json` in the
    /// workspace root, which is where every reader looks for it.
    #[arg(long)]
    pub out: Option<PathBuf>
}

/// Runs the resolver over the workspace and reports what it settled.
///
/// # Errors
///
/// Returns an [`anyhow::Error`] when the report cannot be written.
pub fn run_resolve(args: &ResolveArgs) -> anyhow::Result<ExitCode> {
    let out = args
        .out
        .clone()
        .unwrap_or_else(|| args.path.join("resolution.json"));
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    match settled(&args.path, &out) {
        Ok(()) => {
            let bytes = std::fs::metadata(&out).map_or(0, |held| held.len());
            info!(path = %out.display(), bytes, "resolution written");
            writeln!(handle, "resolution written to {}", out.display())
                .context("writing the resolve report")?;
            Ok(ExitCode::from(EXIT_CLEAN))
        }
        Err(reason) => {
            warn!(%reason, "the resolution was not produced");
            writeln!(handle, "resolution not produced: {reason}")
                .context("writing the resolve report")?;
            Ok(ExitCode::from(EXIT_ERROR))
        }
    }
}

/// Runs the resolver binary over `root`, writing its answer to `out`.
fn settled(root: &std::path::Path, out: &std::path::Path) -> Result<(), String> {
    let output = std::process::Command::new(resolver())
        .arg(root)
        .arg(out)
        .env("CARGO_TERM_COLOR", "never")
        .output()
        .map_err(|error| format!("the resolver could not be invoked: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "the resolver exited with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
            .lines()
            .next_back()
            .unwrap_or("no message")
    ))
}

/// The resolver binary: the one beside this executable where there is one,
/// so a built pair runs without anything being on the path.
fn resolver() -> PathBuf {
    let named = "rustmanifest-resolve";
    std::env::current_exe()
        .ok()
        .and_then(|held| held.parent().map(|beside| beside.join(named)))
        .filter(|beside| beside.is_file())
        .unwrap_or_else(|| PathBuf::from(named))
}
