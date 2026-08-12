// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Library half of the `rustmanifest` CLI binary.
//!
//! The binary in `src/main.rs` is a thin shim that delegates to [`run`],
//! which makes the command surface testable from integration tests without
//! spawning a child process.

#![allow(
    clippy::multiple_crate_versions,
    reason = "the workspace graph carries both syn majors: clap, async-trait, and tokio-macros \
              moved to syn 3 while darling, tracing-attributes, futures-macro, and \
              zerocopy-derive are still on syn 2 — not deduplicable from this crate"
)]

pub mod analyzed;
pub mod ask;
pub mod check;
pub mod coverage;
pub mod document;
pub mod explain;
pub mod facts;
pub mod graph;
pub mod learn;
pub mod manifests;
pub mod meaning;
pub mod overlays;
pub mod overview;
pub mod prescribe;
pub mod report;

use std::{path::PathBuf, process::ExitCode};

use clap::{Parser, Subcommand};

use self::{check::run_check, explain::run_explain};
pub use crate::{
    check::{CheckArgs, OutputFormat, SeverityFilter},
    coverage::CoverageArgs,
    explain::ExplainArgs,
    graph::{DiagramFormat, GraphArgs, GraphKind}
};

/// Default exit code: no findings at or above the requested severity.
pub const EXIT_CLEAN: u8 = 0;
/// Findings present at or above the requested severity.
pub const EXIT_FINDINGS: u8 = 1;
/// Operational failure (IO, argument parsing, walker error).
pub const EXIT_ERROR: u8 = 2;
/// A question the facts refused to answer: the refusal is printed with
/// what was looked for, and a script can tell it from an answer — the
/// same text with exit 0 would make the two indistinguishable.
pub const EXIT_REFUSED: u8 = 3;

/// Top-level CLI arguments.
#[derive(Debug, Parser)]
#[command(
    name = "rustmanifest",
    version,
    about = "Production-grade Rust review engine — methodology-as-code"
)]
pub struct Cli {
    /// Increase verbosity. May be repeated (`-v`, `-vv`, `-vvv`).
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    /// Selected subcommand.
    #[command(subcommand)]
    pub command: Command
}

/// Available subcommands for the `rustmanifest` binary.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Ask why a function carries an effect; the answer is a
    /// step-by-step inference over recorded facts.
    Ask(ask::AskArgs),
    /// Run the analysis engine across one or more paths.
    Check(CheckArgs),
    /// Report how much of the analyzed sources the engine understands.
    Coverage(CoverageArgs),
    /// Print the rationale and methodology excerpt for a rule.
    Explain(ExplainArgs),
    /// Ask the compiler for its documentation index of this workspace.
    Document(document::DocumentArgs),
    /// Print behavioral facts of every function as JSON.
    Facts(facts::FactsArgs),
    /// Print an architecture diagram of the analyzed sources.
    Graph(GraphArgs),
    /// Learn per-role convention clauses and print them as JSON.
    Learn(learn::LearnArgs),

    /// Print what the program does for an observer outside the process.
    Meaning(meaning::MeaningArgs),
    /// Print everything the engine knows about the project as one report.
    Overview(overview::OverviewArgs),
    /// Print the code the canonical form asks for, and what it buys.
    Prescribe(prescribe::PrescribeArgs),
    /// Print everything the engine knows about the program in one pass.
    Report(report::ReportArgs)
}

/// Entry point used by both the binary and integration tests.
///
/// # Errors
///
/// Returns an [`anyhow::Error`] for unrecoverable operational failures.
pub fn run(cli: Cli) -> anyhow::Result<ExitCode> {
    init_tracing(cli.verbose);
    match cli.command {
        Command::Ask(args) => ask::run_ask(&args),
        Command::Check(args) => run_check(&args),
        Command::Coverage(args) => coverage::run_coverage(&args),
        Command::Explain(args) => run_explain(&args),
        Command::Document(args) => document::run_document(&args),
        Command::Facts(args) => facts::run_facts(&args),
        Command::Graph(args) => graph::run_graph(&args),
        Command::Learn(args) => learn::run_learn(&args),
        Command::Meaning(args) => meaning::run_meaning(&args),
        Command::Overview(args) => overview::run_overview(&args),
        Command::Prescribe(args) => prescribe::run_prescribe(&args),
        Command::Report(args) => report::run_report(&args)
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

/// Reads every readable Rust source within the per-file memory budget into
/// memory; unreadable and oversized files are skipped silently.
/// Reads a resolution overlay from `path`, where one was named.
///
/// A named file that cannot be read or parsed is an error and never a
/// silent fallback to no resolution: a caller who asked for resolved
/// answers must not be handed unresolved ones without being told.
fn resolution(
    path: Option<&std::path::Path>
) -> anyhow::Result<Option<rustmanifest_schema::Resolution>> {
    use anyhow::Context as _;

    let Some(path) = path else {
        return Ok(None);
    };
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading the resolution overlay at {}", path.display()))?;
    let settled: rustmanifest_schema::Resolution = serde_json::from_str(&text)
        .with_context(|| format!("parsing the resolution overlay at {}", path.display()))?;
    Ok(Some(settled))
}

fn read_sources(files: &[PathBuf], max_file_bytes: u64) -> Vec<rustmanifest_engine::Source> {
    files
        .iter()
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .filter(|path| {
            std::fs::metadata(path).is_ok_and(|metadata| metadata.len() <= max_file_bytes)
        })
        .filter_map(|path| {
            std::fs::read_to_string(path)
                .ok()
                .map(|text| rustmanifest_engine::Source::new(path.clone(), text))
        })
        .collect()
}
