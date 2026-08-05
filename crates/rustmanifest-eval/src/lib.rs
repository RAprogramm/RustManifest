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
pub mod routes;
pub mod runner;

mod cli;
mod execution;
mod exit;

pub use cli::{BlessArgs, CheckArgs, Cli, Command, RoutesArgs, RunArgs};
pub use error::EvalError;
pub use execution::{P95_BUDGET_MS, run};
pub use exit::{EXIT_CLEAN, EXIT_ERROR, EXIT_REGRESSION};
