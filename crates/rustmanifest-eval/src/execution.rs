// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Execution of the selected subcommand: running the corpus once and turning
//! the resulting report into a written artifact and an exit code.

mod budget;
mod routes;
mod summary;
mod verbosity;

use std::{io::Write, process::ExitCode};

pub use budget::P95_BUDGET_MS;
use budget::budget_breaches;
use summary::write_summary;
use verbosity::init_tracing;

use crate::{
    cli::{BlessArgs, CheckArgs, Cli, Command, RunArgs},
    corpus,
    error::EvalError,
    exit::{EXIT_CLEAN, EXIT_REGRESSION},
    report::{Comparable, Report, diff},
    runner
};

/// Entry point used by both the binary and tests.
///
/// # Errors
///
/// Returns an [`EvalError`] for unrecoverable operational failures;
/// regressions are reported through the exit code instead.
pub fn run(cli: Cli) -> Result<ExitCode, EvalError> {
    init_tracing(cli.verbose);
    match cli.command {
        Command::Run(args) => run_report(&args),
        Command::Check(args) => run_check(&args),
        Command::Bless(args) => run_bless(&args),
        Command::Routes(args) => routes::run_routes(&args)
    }
}

/// Serializes `value` as pretty JSON with a trailing newline.
fn to_pretty_json<T: serde::Serialize>(value: &T) -> Result<String, EvalError> {
    let mut text = serde_json::to_string_pretty(value)?;
    text.push('\n');
    Ok(text)
}

/// Runs the corpus and evaluates it into a full report.
fn evaluate(
    corpus_path: &std::path::Path,
    corpus_root: &std::path::Path
) -> Result<Report, EvalError> {
    let corpus = corpus::load(corpus_path)?;
    runner::evaluate(&corpus, corpus_root)
}

fn run_report(args: &RunArgs) -> Result<ExitCode, EvalError> {
    let report = evaluate(&args.corpus, &args.corpus_root)?;
    let text = to_pretty_json(&report)?;
    std::fs::write(&args.out, text).map_err(|source| EvalError::WriteFile {
        path: args.out.clone(),
        source
    })?;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    write_summary(&mut handle, &report)?;
    writeln!(handle, "report written to {}", args.out.display())?;
    handle.flush()?;
    Ok(ExitCode::from(EXIT_CLEAN))
}

fn run_bless(args: &BlessArgs) -> Result<ExitCode, EvalError> {
    let report = evaluate(&args.corpus, &args.corpus_root)?;
    let text = to_pretty_json(&report.comparable())?;
    std::fs::write(&args.baseline, text).map_err(|source| EvalError::WriteFile {
        path: args.baseline.clone(),
        source
    })?;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    write_summary(&mut handle, &report)?;
    writeln!(handle, "baseline written to {}", args.baseline.display())?;
    handle.flush()?;
    Ok(ExitCode::from(EXIT_CLEAN))
}

fn run_check(args: &CheckArgs) -> Result<ExitCode, EvalError> {
    if !args.baseline.is_file() {
        return Err(EvalError::MissingBaseline {
            path: args.baseline.clone()
        });
    }
    let baseline = load_baseline(&args.baseline)?;
    let report = evaluate(&args.corpus, &args.corpus_root)?;
    let differences = diff(&baseline, &report.comparable());
    let breaches = budget_breaches(&report);

    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    write_summary(&mut handle, &report)?;
    for line in &differences {
        writeln!(handle, "count drift: {line}")?;
    }
    for line in &breaches {
        writeln!(handle, "budget breach: {line}")?;
    }
    let clean = differences.is_empty() && breaches.is_empty();
    if clean {
        writeln!(handle, "check passed: counts match baseline, budgets hold")?;
    } else {
        writeln!(
            handle,
            "check failed: {} count drift(s), {} budget breach(es)",
            differences.len(),
            breaches.len()
        )?;
    }
    handle.flush()?;
    if clean {
        Ok(ExitCode::from(EXIT_CLEAN))
    } else {
        Ok(ExitCode::from(EXIT_REGRESSION))
    }
}

/// Loads the committed baseline from disk.
fn load_baseline(path: &std::path::Path) -> Result<Comparable, EvalError> {
    let text = std::fs::read_to_string(path).map_err(|source| EvalError::BaselineRead {
        path: path.to_path_buf(),
        source
    })?;
    serde_json::from_str(&text).map_err(|source| EvalError::BaselineParse {
        path: path.to_path_buf(),
        source
    })
}
