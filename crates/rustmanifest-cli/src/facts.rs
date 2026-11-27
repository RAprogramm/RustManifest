// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! `facts` subcommand: behavioral fact export.
//!
//! Discovers Rust sources exactly like `check`, runs the engine's
//! behavioral extraction, and prints one JSON object per function: which
//! fields it reads and writes, what it constructs, which error types it
//! can return, and which side-effect categories its body touches. The
//! output is a canonical pretty-printed JSON array, deterministic across
//! runs, meant both for human inspection and as the substrate for
//! invariant rules.

use std::{io::Write, process::ExitCode};

use clap::Parser;
use masterror::ResultExt as _;
use rustmanifest_engine::walker;

use crate::EXIT_CLEAN;

/// Arguments for the `facts` subcommand.
#[derive(Debug, Parser)]
pub struct FactsArgs {
    /// The program under analysis.
    #[command(flatten)]
    pub analyzed: crate::analyzed::Analyzed,

    /// What the run is handed beside the sources.
    #[command(flatten)]
    pub overlays: crate::overlays::Overlays,

    /// Only print functions whose qualified name contains this substring.
    #[arg(long)]
    pub filter: Option<String>,

    /// Only print functions with at least one write, effect, or error
    /// type — skip pure readers and empty bodies.
    #[arg(long)]
    pub active: bool,

    /// Additionally print each function's binarized feature vector — the
    /// fixed-order named binary features the learning layer trains on.
    #[arg(long)]
    pub features: bool
}

/// Runs the `facts` subcommand: discovers files, extracts behavioral
/// facts, and prints them as a JSON array.
pub(crate) fn run_facts(args: &FactsArgs) -> masterror::AppResult<ExitCode> {
    let files = walker::discover(&args.analyzed.paths).context("file discovery failed")?;
    let sources = crate::read_sources(
        &files,
        rustmanifest_engine::orchestrator::DEFAULT_MAX_FILE_BYTES
    );
    let scip = args
        .overlays
        .scip
        .as_deref()
        .map(rustmanifest_engine::ScipEdges::from_file)
        .transpose()
        .context("loading SCIP index")?;
    let resolution = args.analyzed.resolution()?;
    let rendered =
        serde_json::to_string_pretty(&rustmanifest_engine::facts_export::facts_json_with(
            &sources,
            resolution.as_ref(),
            args.filter.as_deref(),
            args.active,
            args.features,
            scip.as_ref()
        ))
        .context("serializing facts")?;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    writeln!(handle, "{rendered}").context("writing facts output")?;
    handle.flush().context("flushing stdout")?;
    Ok(ExitCode::from(EXIT_CLEAN))
}
