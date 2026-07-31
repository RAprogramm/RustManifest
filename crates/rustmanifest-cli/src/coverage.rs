// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! `coverage` subcommand: self-measurement of analysis understanding.
//!
//! Discovers Rust sources exactly like `check`, runs the engine's
//! [`coverage`] measurement over them, and prints a compact aligned report
//! of raw counters and rates: parsed files, modules mapped, structures and
//! fields captured, `use` target resolution, and macro sightings. The
//! report never overclaims: derive and macro expansions are not analyzed,
//! and the output says so.

use std::{io::Write, path::PathBuf, process::ExitCode};

use anyhow::Context;
use clap::Parser;
use rustmanifest_engine::{Coverage, coverage, walker};

use crate::EXIT_CLEAN;

/// Arguments for the `coverage` subcommand.
#[derive(Debug, Parser)]
pub struct CoverageArgs {
    /// Files or directories to analyze. Defaults to the current directory.
    #[arg(default_value = ".")]
    pub paths: Vec<PathBuf>
}

/// Runs the `coverage` subcommand: discovers files, reads sources within
/// the default per-file memory budget, and prints the coverage report.
pub(crate) fn run_coverage(args: &CoverageArgs) -> anyhow::Result<ExitCode> {
    let files = walker::discover(&args.paths).context("file discovery failed")?;
    let sources = crate::read_sources(
        &files,
        rustmanifest_engine::orchestrator::DEFAULT_MAX_FILE_BYTES
    );
    let report = coverage(&sources);
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    for line in report_lines(&report) {
        writeln!(handle, "{line}").context("writing coverage output")?;
    }
    handle.flush().context("flushing stdout")?;
    Ok(ExitCode::from(EXIT_CLEAN))
}

/// Renders the aligned coverage report, one output line per element.
fn report_lines(report: &Coverage) -> Vec<String> {
    let parsed = format!("{}/{}", report.files_parsed, report.files_total);
    let reachable = format!("{}/{}", report.files_reachable, report.files_total);
    let parse_rate = report.parse_rate();
    let reach_rate = report.reachability_rate();
    let resolution_rate = report.resolution_rate();
    let fields = report.fields_captured;
    let edges = report.aggregation_edges_resolved;
    let resolved = report.use_targets_resolved;
    let external = report.use_targets_external;
    let macros = report.item_macros_seen;
    vec![
        line("files parsed", &parsed, &format!("{parse_rate:.1}%")),
        line(
            "modules mapped",
            &report.modules_total.to_string(),
            &format!("(files reachable from crate roots: {reachable}  {reach_rate:.1}%)")
        ),
        line(
            "structs captured",
            &report.structs_captured.to_string(),
            &format!("fields {fields}   aggregation edges {edges}")
        ),
        line(
            "use targets",
            &report.use_targets_total.to_string(),
            &format!(
                "resolved in-workspace {resolved}  {resolution_rate:.1}%   \
                 external/unresolved {external}"
            )
        ),
        line(
            "derives seen",
            &report.derives_seen.to_string(),
            &format!("item macros seen {macros}   (expansions not analyzed)")
        ),
    ]
}

/// Formats one report line: a padded label column, a padded value column,
/// and a free-form tail, without trailing whitespace.
fn line(label: &str, value: &str, tail: &str) -> String {
    format!("{label:<20}{value:<10}{tail}")
        .trim_end()
        .to_owned()
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use rustmanifest_engine::Coverage;

    use super::report_lines;

    #[test]
    fn report_lines_are_aligned_and_honest() {
        let report = Coverage {
            files_total:                318,
            files_parsed:               318,
            modules_total:              212,
            files_reachable:            296,
            structs_captured:           509,
            fields_captured:            2140,
            aggregation_edges_resolved: 87,
            use_targets_total:          4044,
            use_targets_resolved:       1620,
            use_targets_external:       2424,
            derives_seen:               233,
            item_macros_seen:           12
        };
        let lines = report_lines(&report);
        assert_eq!(lines.len(), 5);
        assert_eq!(lines[0], "files parsed        318/318   100.0%");
        assert_eq!(
            lines[1],
            "modules mapped      212       (files reachable from crate roots: 296/318  93.1%)"
        );
        assert_eq!(
            lines[2],
            "structs captured    509       fields 2140   aggregation edges 87"
        );
        assert_eq!(
            lines[3],
            "use targets         4044      resolved in-workspace 1620  40.1%   \
             external/unresolved 2424"
        );
        assert_eq!(
            lines[4],
            "derives seen        233       item macros seen 12   (expansions not analyzed)"
        );
    }

    #[test]
    fn empty_report_renders_zero_rates() {
        let lines = report_lines(&Coverage::default());
        assert_eq!(lines[0], "files parsed        0/0       0.0%");
    }
}
