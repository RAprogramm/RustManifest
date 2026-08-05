// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! `learn` subcommand: interpretable convention learning.
//!
//! Discovers Rust sources exactly like `check`, trains one deterministic
//! Tsetlin machine per behavioral role on the engine's binarized feature
//! vectors (bootstrap labels come from the role heuristic), and prints
//! every learned positive clause as a JSON object: the role it votes for,
//! the features that must hold, and the features that must not. The
//! output is an auditable rule list, deterministic across runs; it
//! describes the codebase and is never a source of findings.

use std::{io::Write, path::PathBuf, process::ExitCode};

use anyhow::Context;
use clap::Parser;
use rustmanifest_learn::{LearnOptions, learn_roles};

use crate::EXIT_CLEAN;

/// Arguments for the `learn` subcommand.
#[derive(Debug, Parser)]
pub struct LearnArgs {
    /// Files or directories to analyze. Defaults to the current directory.
    #[arg(default_value = ".")]
    pub paths: Vec<PathBuf>,

    /// Clauses per role machine.
    #[arg(long, default_value_t = 20)]
    pub clauses: usize,

    /// Training epochs.
    #[arg(long, default_value_t = 30)]
    pub epochs: usize,

    /// Training seed; identical seeds yield identical clauses.
    #[arg(long, default_value_t = 42)]
    pub seed: u64
}

/// Runs the `learn` subcommand: discovers files, trains per-role
/// machines, and prints the learned clauses as a JSON array.
pub(crate) fn run_learn(args: &LearnArgs) -> anyhow::Result<ExitCode> {
    let files =
        rustmanifest_engine::walker::discover(&args.paths).context("file discovery failed")?;
    let sources = crate::read_sources(
        &files,
        rustmanifest_engine::orchestrator::DEFAULT_MAX_FILE_BYTES
    );
    let options = LearnOptions {
        clauses: args.clauses,
        epochs: args.epochs,
        seed: args.seed,
        ..LearnOptions::default()
    };
    let learned = learn_roles(&rustmanifest_engine::Workspace::new(&sources), options);
    let rendered = serde_json::to_string_pretty(
        &learned
            .iter()
            .map(|clause| {
                serde_json::json!({
                    "role": clause.role,
                    "if": clause.includes,
                    "unless": clause.excludes
                })
            })
            .collect::<Vec<_>>()
    )
    .context("serializing learned clauses")?;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    writeln!(handle, "{rendered}").context("writing learn output")?;
    handle.flush().context("flushing stdout")?;
    Ok(ExitCode::from(EXIT_CLEAN))
}
