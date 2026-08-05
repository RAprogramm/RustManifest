// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! `overview` subcommand: everything the engine knows, in one report.
//!
//! Discovers Rust sources exactly like `check`, learns per-role
//! convention clauses, and prints the engine's deterministic Markdown
//! knowledge report to stdout.

use std::{io::Write, path::PathBuf, process::ExitCode};

use anyhow::Context;
use clap::Parser;
use rustmanifest_engine::overview::{Convention, overview_markdown};
use rustmanifest_learn::{LearnOptions, learn_roles};

use crate::EXIT_CLEAN;

/// Arguments for the `overview` subcommand.
#[derive(Debug, Parser)]
pub struct OverviewArgs {
    /// Files or directories to analyze. Defaults to the current directory.
    #[arg(default_value = ".")]
    pub paths: Vec<PathBuf>,

    /// Cap for every ranked list in the report.
    #[arg(long, default_value_t = 10)]
    pub top: usize
}

/// Runs the `overview` subcommand: analyzes the paths and prints the
/// aggregated knowledge report.
pub(crate) fn run_overview(args: &OverviewArgs) -> anyhow::Result<ExitCode> {
    let files =
        rustmanifest_engine::walker::discover(&args.paths).context("file discovery failed")?;
    let sources = crate::read_sources(
        &files,
        rustmanifest_engine::orchestrator::DEFAULT_MAX_FILE_BYTES
    );
    let workspace = rustmanifest_engine::Workspace::new(&sources);
    let conventions: Vec<Convention> = learn_roles(&workspace, LearnOptions::default())
        .into_iter()
        .map(|clause| Convention {
            role:     clause.role,
            includes: clause.includes,
            excludes: clause.excludes
        })
        .collect();
    let report = overview_markdown(&workspace, args.top, &conventions);
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    write!(handle, "{report}").context("writing overview output")?;
    handle.flush().context("flushing stdout")?;
    Ok(ExitCode::from(EXIT_CLEAN))
}
