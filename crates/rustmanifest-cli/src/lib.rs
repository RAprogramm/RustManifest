// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Library half of the `rustmanifest` CLI binary.
//!
//! The binary in `src/main.rs` is a thin shim that delegates to [`run`],
//! which makes the command surface testable from integration tests without
//! spawning a child process.

pub mod check;
pub mod coverage;
pub mod expand;
pub mod explain;
pub mod facts;
pub mod graph;
pub mod learn;
pub mod manifests;
pub mod overview;

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
    /// Run the analysis engine across one or more paths.
    Check(CheckArgs),
    /// Report how much of the analyzed sources the engine understands.
    Coverage(CoverageArgs),
    /// Print the rationale and methodology excerpt for a rule.
    Explain(ExplainArgs),
    /// Print behavioral facts of every function as JSON.
    Facts(facts::FactsArgs),
    /// Print an architecture diagram of the analyzed sources.
    Graph(GraphArgs),
    /// Learn per-role convention clauses and print them as JSON.
    Learn(learn::LearnArgs),
    /// Print everything the engine knows about the project as one report.
    Overview(overview::OverviewArgs)
}

/// Entry point used by both the binary and integration tests.
///
/// # Errors
///
/// Returns an [`anyhow::Error`] for unrecoverable operational failures.
pub fn run(cli: Cli) -> anyhow::Result<ExitCode> {
    init_tracing(cli.verbose);
    match cli.command {
        Command::Check(args) => run_check(&args),
        Command::Coverage(args) => coverage::run_coverage(&args),
        Command::Explain(args) => run_explain(&args),
        Command::Facts(args) => facts::run_facts(&args),
        Command::Graph(args) => graph::run_graph(&args),
        Command::Learn(args) => learn::run_learn(&args),
        Command::Overview(args) => overview::run_overview(&args)
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
