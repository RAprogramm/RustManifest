// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Library half of the `rustmanifest-eval` binary.
//!
//! The evaluation harness runs the full engine composition over a pinned
//! corpus of repositories and turns the results into a regression gate:
//!
//! - `run` writes a full report including timings.
//! - `bless` records the comparable section as the committed baseline.
//! - `check` re-runs the corpus and fails on any count drift from the baseline
//!   or any p95 per-file budget breach.
#![allow(
    clippy::multiple_crate_versions,
    reason = "duplicate transitive versions are pulled in by upstream dependencies"
)]

pub mod corpus;
pub mod error;
pub mod report;
pub mod runner;

use std::{io::Write, path::PathBuf, process::ExitCode};

use clap::{Parser, Subcommand};
pub use error::EvalError;
use report::{Comparable, Report, diff};

/// Exit code: counts match the baseline and every budget holds.
pub const EXIT_CLEAN: u8 = 0;
/// Exit code: at least one count drifted or a performance budget broke.
pub const EXIT_REGRESSION: u8 = 1;
/// Exit code: operational failure (IO, parsing, engine error).
pub const EXIT_ERROR: u8 = 2;

/// Per-file p95 budget in milliseconds for files under 1000 lines.
pub const P95_BUDGET_MS: f64 = 100.0;

/// Top-level CLI arguments.
#[derive(Debug, Parser)]
#[command(
    name = "rustmanifest-eval",
    version,
    about = "Evaluation harness for the rustmanifest engine"
)]
pub struct Cli {
    /// Increase verbosity. May be repeated (`-v`, `-vv`, `-vvv`).
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    /// Selected subcommand.
    #[command(subcommand)]
    pub command: Command
}

/// Available subcommands for the `rustmanifest-eval` binary.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run the corpus and write a full report including timings.
    Run(RunArgs),
    /// Run the corpus and compare against the committed baseline.
    Check(CheckArgs),
    /// Run the corpus and overwrite the committed baseline.
    Bless(BlessArgs)
}

/// Arguments for the `run` subcommand.
#[derive(Debug, Parser)]
pub struct RunArgs {
    /// Directory the corpus entry paths are resolved against.
    #[arg(long)]
    pub corpus_root: PathBuf,

    /// Corpus manifest location.
    #[arg(long, default_value = "eval/corpus.toml")]
    pub corpus: PathBuf,

    /// Report output location.
    #[arg(long, default_value = "eval/report.json")]
    pub out: PathBuf
}

/// Arguments for the `check` subcommand.
#[derive(Debug, Parser)]
pub struct CheckArgs {
    /// Directory the corpus entry paths are resolved against.
    #[arg(long)]
    pub corpus_root: PathBuf,

    /// Corpus manifest location.
    #[arg(long, default_value = "eval/corpus.toml")]
    pub corpus: PathBuf,

    /// Committed baseline location.
    #[arg(long, default_value = "eval/baseline.json")]
    pub baseline: PathBuf
}

/// Arguments for the `bless` subcommand.
#[derive(Debug, Parser)]
pub struct BlessArgs {
    /// Directory the corpus entry paths are resolved against.
    #[arg(long)]
    pub corpus_root: PathBuf,

    /// Corpus manifest location.
    #[arg(long, default_value = "eval/corpus.toml")]
    pub corpus: PathBuf,

    /// Baseline location to write.
    #[arg(long, default_value = "eval/baseline.json")]
    pub baseline: PathBuf
}

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
        Command::Bless(args) => run_bless(&args)
    }
}

fn init_tracing(verbose: u8) {
    let level = match verbose {
        0 => tracing::Level::WARN,
        1 => tracing::Level::INFO,
        2 => tracing::Level::DEBUG,
        _ => tracing::Level::TRACE
    };
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(level)
        .with_target(false)
        .with_writer(std::io::stderr)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);
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

/// Lists every repository whose p95 per-file time over files under 1000
/// lines exceeds [`P95_BUDGET_MS`].
fn budget_breaches(report: &Report) -> Vec<String> {
    report
        .timings
        .iter()
        .filter(|(_, timings)| timings.p95_under_1k_ms > P95_BUDGET_MS)
        .map(|(name, timings)| {
            format!(
                "repo {name}: p95 {:.2} ms over files under 1k lines exceeds {P95_BUDGET_MS} ms",
                timings.p95_under_1k_ms
            )
        })
        .collect()
}

/// Writes the compact per-repository summary onto `handle`.
fn write_summary(handle: &mut impl Write, report: &Report) -> Result<(), EvalError> {
    for repo in &report.repos {
        let total: u64 = repo.findings.values().sum();
        let p95 = report
            .timings
            .get(&repo.name)
            .map_or(0.0, |timings| timings.p95_under_1k_ms);
        writeln!(
            handle,
            "{}: {} files, {} lines, {} finding(s), p95 {:.2} ms",
            repo.name, repo.files, repo.rust_lines, total, p95
        )?;
    }
    Ok(())
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::report::{RepoReport, RepoTimings};

    fn report_with_p95(p95: f64) -> Report {
        Report {
            engine_version: "0.0.0".to_owned(),
            repos:          vec![RepoReport {
                name:       "alpha".to_owned(),
                pinned_rev: "abc".to_owned(),
                actual_rev: "abc".to_owned(),
                files:      1,
                rust_lines: 10,
                findings:   BTreeMap::new()
            }],
            timings:        BTreeMap::from([(
                "alpha".to_owned(),
                RepoTimings {
                    p95_under_1k_ms: p95,
                    files_under_1k:  1,
                    max_file_ms:     p95,
                    semantic_ms:     0.0,
                    total_ms:        p95
                }
            )])
        }
    }

    #[test]
    fn budget_breach_is_reported_above_the_limit() {
        let breaches = budget_breaches(&report_with_p95(100.5));
        assert_eq!(breaches.len(), 1);
        assert!(breaches[0].contains("repo alpha"));
        assert!(breaches[0].contains("100.50 ms"));
    }

    #[test]
    fn budget_holds_at_or_below_the_limit() {
        assert!(budget_breaches(&report_with_p95(100.0)).is_empty());
        assert!(budget_breaches(&report_with_p95(3.0)).is_empty());
    }

    #[test]
    fn summary_writes_one_line_per_repo() {
        let mut buffer = Vec::new();
        write_summary(&mut buffer, &report_with_p95(1.0)).unwrap();
        let text = String::from_utf8(buffer).unwrap();
        assert_eq!(
            text,
            "alpha: 1 files, 10 lines, 0 finding(s), p95 1.00 ms\n"
        );
    }
}
