// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! `graph` subcommand: architecture-diagram export.
//!
//! Discovers Rust sources exactly like `check`, builds the requested view —
//! module dependencies via [`module_overview`] or type aggregation via
//! [`TypeGraph`] — and prints a Mermaid or Graphviz DOT diagram to stdout.
//! Output is deterministic: nodes and edges are sorted and deduplicated.

mod modules;
mod options;
mod types;

use std::{io::Write, process::ExitCode};

use anyhow::Context;
use rustmanifest_engine::{TypeGraph, module_overview, walker};

pub use self::options::{DiagramFormat, GraphArgs, GraphKind};
use self::{modules::module_lines, types::type_lines};
use crate::EXIT_CLEAN;

/// Runs the `graph` subcommand: discovers files, reads sources within the
/// default per-file memory budget, and prints the requested diagram.
pub(crate) fn run_graph(args: &GraphArgs) -> anyhow::Result<ExitCode> {
    let files = walker::discover(&args.paths).context("file discovery failed")?;
    let sources = crate::read_sources(
        &files,
        rustmanifest_engine::orchestrator::DEFAULT_MAX_FILE_BYTES
    );
    let settled = crate::resolution(
            args.resolution.as_deref(),
            args.paths.first().map_or(std::path::Path::new("."), std::path::PathBuf::as_path)
        )?;
    let workspace = settled.as_ref().map_or_else(
        || rustmanifest_engine::Workspace::new(&sources),
        |resolution| rustmanifest_engine::Workspace::resolved(&sources, resolution)
    );
    let lines = match args.kind {
        GraphKind::Modules => module_lines(&module_overview(&workspace), args.format),
        GraphKind::Types => type_lines(&TypeGraph::from_sources(&sources), args.format)
    };
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    for line in lines {
        writeln!(handle, "{line}").context("writing diagram output")?;
    }
    handle.flush().context("flushing stdout")?;
    Ok(ExitCode::from(EXIT_CLEAN))
}
