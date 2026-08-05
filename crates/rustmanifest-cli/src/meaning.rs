// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! `meaning` subcommand: what the analyzed program does for an observer
//! outside the process.
//!
//! The default rendering is a table meant to be read: one block per
//! entry point, one aligned row per write — how certainly it happens,
//! which stream it goes to, the text it puts there, and the function
//! that does it. Properties of the standard library rather than of the
//! analyzed program are stated once as a footnote, not repeated on every
//! row. `--format json` emits the same claims as data, for tools.

mod completion;
mod data;
mod location;
mod options;
mod table;
mod wording;

use std::{io::Write, process::ExitCode};

use anyhow::Context;
use rustmanifest_engine::walker;

pub use self::options::{MeaningArgs, MeaningFormat};
use self::{data::json_report, table::text_report};
use crate::EXIT_CLEAN;

/// Runs the `meaning` subcommand.
pub(crate) fn run_meaning(args: &MeaningArgs) -> anyhow::Result<ExitCode> {
    let files = walker::discover(&args.paths).context("file discovery failed")?;
    let sources = crate::read_sources(
        &files,
        rustmanifest_engine::orchestrator::DEFAULT_MAX_FILE_BYTES
    );
    let entries =
        rustmanifest_engine::program_meaning(&rustmanifest_engine::Workspace::new(&sources));
    let root = args.paths.first().cloned().unwrap_or_default();
    let rendered = match args.format {
        MeaningFormat::Text => text_report(&entries, &root),
        MeaningFormat::Json => serde_json::to_string_pretty(&json_report(&entries, &root))
            .context("serializing meaning report")?
    };
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    writeln!(handle, "{rendered}").context("writing meaning output")?;
    handle.flush().context("flushing stdout")?;
    Ok(ExitCode::from(EXIT_CLEAN))
}
