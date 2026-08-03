// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! `report` subcommand: everything the engine knows about a program, in
//! one pass.
//!
//! The knowledge is spread over five surfaces — what the sources hold,
//! what the program does for an observer outside the process, the facts
//! behind that, the judgements against the canonical forms, and how much
//! of the sources the engine could read at all. Running them one by one
//! makes the reader assemble the picture; this subcommand assembles it
//! and prints the honesty section last, so a reader always sees what the
//! rest of the report is worth.

mod behaviour;
mod findings;
mod honesty;
mod judgement;
mod location;
mod options;
mod structure;

use std::{io::Write, process::ExitCode};

use anyhow::Context;

pub use self::options::ReportArgs;
use self::{behaviour::behaviour, honesty::honesty, judgement::judgement, structure::structure};
use crate::EXIT_CLEAN;

/// Runs the `report` subcommand.
pub(crate) fn run_report(args: &ReportArgs) -> anyhow::Result<ExitCode> {
    let files =
        rustmanifest_engine::walker::discover(&args.paths).context("file discovery failed")?;
    let sources = crate::read_sources(
        &files,
        rustmanifest_engine::orchestrator::DEFAULT_MAX_FILE_BYTES
    );
    let root = args.paths.first().cloned().unwrap_or_default();
    let mut out = String::new();
    out.push_str(&behaviour(&sources, &root));
    out.push_str(&judgement(&sources, &root));
    out.push_str(&structure(&sources, args.top));
    out.push_str(&honesty(&sources));
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    write!(handle, "{out}").context("writing report output")?;
    handle.flush().context("flushing stdout")?;
    Ok(ExitCode::from(EXIT_CLEAN))
}
