// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! `coverage` subcommand: self-measurement of analysis understanding.
//!
//! Discovers Rust sources exactly like `check`, runs the engine's
//! [`coverage`] measurement over them, and prints a compact aligned report
//! of raw counters and rates: parsed files, modules mapped, structures and
//! fields captured, `use` target resolution, and macro sightings. The
//! report never overclaims: derive and macro expansions are not analyzed
//! by default, and the output says so. With `--expand`, every discovered
//! crate is additionally expanded via nightly `rustc` and the report gains
//! a block measuring what the derives actually generate.

mod counters;
mod dependencies;
mod expansion;
mod layout;
mod measurement;
mod options;
mod unresolved;

use std::{io::Write, process::ExitCode};

use anyhow::Context;

pub use self::options::CoverageArgs;
use self::{
    counters::report_lines,
    expansion::{expand_lines, integration_line},
    measurement::measure,
    unresolved::unresolved_lines
};
use crate::EXIT_CLEAN;

/// Runs the `coverage` subcommand: discovers files, reads sources within
/// the default per-file memory budget, and prints the coverage report.
pub(crate) fn run_coverage(args: &CoverageArgs) -> anyhow::Result<ExitCode> {
    let measured = measure(args)?;
    let mut lines = report_lines(&measured.report);
    if args.unresolved {
        lines.extend(unresolved_lines(&rustmanifest_engine::unresolved_targets(
            &measured.sources,
            &measured.declared
        )));
    }
    if args.expand {
        let summary = crate::expand::expand_paths(&args.paths);
        lines.extend(expand_lines(&summary));
        lines.push(integration_line(&measured.sources, &summary));
    }
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    for line in lines {
        writeln!(handle, "{line}").context("writing coverage output")?;
    }
    handle.flush().context("flushing stdout")?;
    Ok(ExitCode::from(EXIT_CLEAN))
}
