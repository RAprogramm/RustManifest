// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! `check` subcommand: the full analysis pipeline.
//!
//! Loads configuration, discovers files, runs all three tiers through the
//! orchestrator, merges workspace-level semantic findings, optionally
//! delegates to `cargo clippy`, applies severity overrides and the
//! severity filter, renders in the requested format, and maps the outcome
//! to the documented exit codes.

mod delegation;
mod fixes;
mod globs;
mod options;
mod output;
mod pipeline;
mod severity;

use std::process::ExitCode;

use tracing::info;

use self::{
    fixes::apply_machine_fixes, output::render, pipeline::collect_findings,
    severity::severity_rank
};
pub use self::{options::CheckArgs, output::OutputFormat, severity::SeverityFilter};
use crate::{EXIT_CLEAN, EXIT_FINDINGS};

pub(crate) fn run_check(args: &CheckArgs) -> masterror::AppResult<ExitCode> {
    let (mut findings, effective) = collect_findings(args)?;
    if args.fix {
        let applied = apply_machine_fixes(&findings)?;
        if applied > 0 {
            info!(
                applied,
                "machine-applicable fixes applied; re-running analysis"
            );
            (findings, _) = collect_findings(args)?;
        }
    }

    render(&findings, args.format, args.no_color)?;

    let threshold = severity_rank(effective.exit_threshold());
    let exit_relevant = findings
        .iter()
        .any(|finding| severity_rank(finding.severity) >= threshold);
    if exit_relevant {
        Ok(ExitCode::from(EXIT_FINDINGS))
    } else {
        Ok(ExitCode::from(EXIT_CLEAN))
    }
}
