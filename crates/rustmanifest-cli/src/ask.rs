// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! `ask` subcommand: a question about the analyzed program, answered as
//! a step-by-step inference over recorded facts.
//!
//! The first question form: why does a function carry an effect? The
//! answer prints each step of the derivation with the reason it is
//! believed, ending at the written call that proved the effect — or a
//! refusal naming exactly what the facts could not answer.

use std::{io::Write, process::ExitCode};

use anyhow::Context;
use clap::Args;
use rustmanifest_engine::walker;

use crate::{EXIT_CLEAN, EXIT_REFUSED};

/// Arguments of the `ask` subcommand.
#[derive(Debug, Args)]
pub struct AskArgs {
    /// Function the question is about, by qualified or bare name.
    pub function: String,

    /// Effect asked about: `fs`, `net`, `process`, `env`, `console`,
    /// `db-read`, `db-write`, `db` (a statement provably sent with its
    /// kind unproven), or `unsafe`.
    pub effect: String,

    /// The program under analysis.
    #[command(flatten)]
    pub analyzed: crate::analyzed::Analyzed,
}

/// Runs the `ask` subcommand.
pub(crate) fn run_ask(args: &AskArgs) -> anyhow::Result<ExitCode> {
    let files = walker::discover(&args.analyzed.paths).context("file discovery failed")?;
    let sources = crate::read_sources(
        &files,
        rustmanifest_engine::orchestrator::DEFAULT_MAX_FILE_BYTES,
    );
    let resolution = args.analyzed.resolution()?;
    let workspace = resolution.as_ref().map_or_else(
        || rustmanifest_engine::Workspace::new(&sources),
        |settled| rustmanifest_engine::Workspace::resolved(&sources, settled),
    );
    let answer =
        rustmanifest_engine::semantic::why_effect(&workspace, &args.function, &args.effect);
    let stdout = std::io::stdout();
    let mut handle = std::io::BufWriter::new(stdout.lock());
    for (position, step) in answer.steps.iter().enumerate() {
        writeln!(handle, "{}. {}  [{}]", position + 1, step.fact, step.reason)
            .context("writing answer step")?;
    }
    writeln!(handle, "=> {}", answer.conclusion).context("writing conclusion")?;
    handle.flush().context("flushing stdout")?;
    if answer.refused {
        return Ok(ExitCode::from(EXIT_REFUSED));
    }
    Ok(ExitCode::from(EXIT_CLEAN))
}
