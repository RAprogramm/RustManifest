// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Command-line surface of the evaluation harness: the top-level arguments,
//! the subcommand set, and the arguments each subcommand accepts.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

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
    Bless(BlessArgs),
    /// Measure the recovered route table against the one each project
    /// declares for `OpenAPI` generation.
    Routes(RoutesArgs)
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

/// Arguments for the `routes` subcommand.
#[derive(Debug, Parser)]
pub struct RoutesArgs {
    /// Directory the corpus entry paths are resolved against.
    #[arg(long)]
    pub corpus_root: PathBuf,

    /// Corpus manifest location.
    #[arg(long, default_value = "eval/corpus.toml")]
    pub corpus: PathBuf,

    /// Measurement output location.
    #[arg(long, default_value = "eval/routes.json")]
    pub out: PathBuf
}
