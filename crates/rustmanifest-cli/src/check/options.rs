// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Command-line surface of the `check` subcommand.

use std::path::PathBuf;

use clap::Parser;

use super::{output::OutputFormat, severity::SeverityFilter};

/// Arguments for the `check` subcommand.
#[derive(Debug, Parser)]
pub struct CheckArgs {
    /// Files or directories to analyze. Defaults to the current directory.
    #[arg(default_value = ".")]
    pub paths: Vec<PathBuf>,

    /// Output format.
    #[arg(long, value_enum, default_value_t = OutputFormat::Auto)]
    pub format: OutputFormat,

    /// Minimum severity to report.
    #[arg(long, value_enum, default_value_t = SeverityFilter::Hint)]
    pub severity_filter: SeverityFilter,

    /// Force-disable color output even on a TTY.
    #[arg(long)]
    pub no_color: bool,

    /// Per-file memory budget in bytes.
    #[arg(long, default_value_t = rustmanifest_engine::orchestrator::DEFAULT_MAX_FILE_BYTES)]
    pub max_file_size: u64,

    /// Path to a `rustmanifest.toml` configuration file. Defaults to
    /// `rustmanifest.toml` in the current directory when present.
    #[arg(long)]
    pub config: Option<PathBuf>,

    /// Directory for the persistent findings cache. Off by default; the
    /// directory is created when missing. Entries are content-addressed by
    /// file bytes, enabled rule set, and engine version, so stale entries
    /// are never served. The workspace-level semantic tier is not cached.
    #[arg(long, value_name = "DIR")]
    pub cache: Option<PathBuf>,

    /// Delegate to `cargo clippy` and surface its diagnostics as advisory
    /// findings alongside rustmanifest's own, carrying clippy's severities
    /// and linking each finding to the clippy lint index as its rationale.
    ///
    /// Requires a `Cargo.toml` directly inside the first analyzed path; the
    /// probe is deliberately shallow and parent workspace manifests are not
    /// searched. Delegation is best-effort and never fails the run: a
    /// missing `cargo` binary, a non-zero `cargo clippy` exit without
    /// parseable output, or malformed JSON lines are each reported as a
    /// warning, while findings from parseable lines are still used.
    #[arg(long)]
    pub delegate_clippy: bool,

    /// Apply machine-applicable fixes to the analyzed files, then re-run
    /// the analysis so the output reflects the corrected sources. Only
    /// fixes proven safe are applied; everything else stays a suggestion.
    #[arg(long)]
    pub fix: bool,

    /// SCIP index file (`rust-analyzer scip .`) whose name-resolved call
    /// edges widen the graph declared invariants are checked against, so
    /// a ban also fires through calls the syntactic resolver cannot bind
    /// — macro-generated bodies, trait dispatch, untyped receivers. Every
    /// trace step names the provenance of the edge it walks.
    ///
    /// The index must be built over the same cfg universe as the analyzed
    /// code (`rust-analyzer` indexes active code only; feature-gated
    /// bodies need the features on) and covers workspace code only, so
    /// edges into dependencies stay outside it. The overlay is additive:
    /// it can only add findings, never remove one.
    #[arg(long, value_name = "INDEX")]
    pub scip: Option<PathBuf>,

    /// Resolution overlay to read from. Module-scoped invariants need one:
    /// which module a file belongs to is the compiler's answer.
    #[arg(long)]
    pub resolution: Option<PathBuf>,
}
