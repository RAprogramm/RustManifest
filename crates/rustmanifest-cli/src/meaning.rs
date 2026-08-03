// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! `meaning` subcommand: what the analyzed program does for an observer
//! outside the process.
//!
//! Prints one sentence per recorded write reachable from each entry
//! point, each degraded exactly as far as the recorded facts allow. A
//! program with no recognized write says so in the words of the
//! recognizer, never as a claim that the program is silent.

use std::{io::Write, path::PathBuf, process::ExitCode};

use anyhow::Context;
use clap::Parser;
use rustmanifest_engine::walker;

use crate::EXIT_CLEAN;

/// Arguments for the `meaning` subcommand.
#[derive(Debug, Parser)]
pub struct MeaningArgs {
    /// Files or directories to analyze. Defaults to the current directory.
    #[arg(default_value = ".")]
    pub paths: Vec<PathBuf>
}

/// Runs the `meaning` subcommand.
pub(crate) fn run_meaning(args: &MeaningArgs) -> anyhow::Result<ExitCode> {
    let files = walker::discover(&args.paths).context("file discovery failed")?;
    let sources = crate::read_sources(
        &files,
        rustmanifest_engine::orchestrator::DEFAULT_MAX_FILE_BYTES
    );
    let entries = rustmanifest_engine::program_meaning(&sources);
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    if entries.is_empty() {
        writeln!(
            handle,
            "no process entry point found — this source slice declares no program"
        )
        .context("writing meaning output")?;
        handle.flush().context("flushing stdout")?;
        return Ok(ExitCode::from(EXIT_CLEAN));
    }
    for entry in &entries {
        writeln!(handle, "# {}", entry.file).context("writing meaning output")?;
        for sentence in entry.sentences() {
            writeln!(handle, "- {sentence}").context("writing meaning output")?;
        }
    }
    handle.flush().context("flushing stdout")?;
    Ok(ExitCode::from(EXIT_CLEAN))
}
