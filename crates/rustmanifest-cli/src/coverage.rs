// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! `coverage` subcommand: self-measurement of analysis understanding.
//!
//! Discovers Rust sources exactly like `check`, runs the engine's
//! [`coverage`] measurement over them, and prints a compact aligned report
//! of raw counters and rates: parsed files, modules mapped, structures and
//! fields captured, `use` target resolution, and macro sightings. The
//! report never overclaims: derive and macro expansions are not analyzed
//! by default, and the output says so. With `--expand`, every discovered
//! crate is additionally expanded via nightly `rustc` and the report gains
//! a block measuring what the derives actually generate.

use std::{collections::BTreeSet, io::Write, path::PathBuf, process::ExitCode};

use anyhow::Context;
use clap::Parser;
use rustmanifest_engine::{Coverage, coverage_with, walker};

use crate::{EXIT_CLEAN, expand::ExpandSummary, manifests};

/// Arguments for the `coverage` subcommand.
#[derive(Debug, Parser)]
pub struct CoverageArgs {
    /// Files or directories to analyze. Defaults to the current directory.
    #[arg(default_value = ".")]
    pub paths: Vec<PathBuf>,

    /// Also expand every crate via `cargo +nightly rustc -- \
    /// -Zunpretty=expanded` and report what derives generate.
    #[arg(long)]
    pub expand: bool
}

/// Runs the `coverage` subcommand: discovers files, reads sources within
/// the default per-file memory budget, and prints the coverage report.
pub(crate) fn run_coverage(args: &CoverageArgs) -> anyhow::Result<ExitCode> {
    let files = walker::discover(&args.paths).context("file discovery failed")?;
    let sources = crate::read_sources(
        &files,
        rustmanifest_engine::orchestrator::DEFAULT_MAX_FILE_BYTES
    );
    let report = coverage_with(&sources, &declared_dependencies(&args.paths));
    let mut lines = report_lines(&report);
    if args.expand {
        lines.extend(expand_lines(&crate::expand::expand_paths(&args.paths)));
    }
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    for line in lines {
        writeln!(handle, "{line}").context("writing coverage output")?;
    }
    handle.flush().context("flushing stdout")?;
    Ok(ExitCode::from(EXIT_CLEAN))
}

/// Collects dependency names declared in every manifest under `paths`,
/// in their in-code spelling, so the coverage report can attribute
/// non-workspace `use` targets to known external crates.
fn declared_dependencies(paths: &[PathBuf]) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for manifest in manifests::manifest_files(paths) {
        if let Ok(text) = std::fs::read_to_string(&manifest) {
            names.extend(manifests::dependency_names(&text));
        }
    }
    names
}

/// Renders the `--expand` report block appended after the coverage lines.
fn expand_lines(summary: &ExpandSummary) -> Vec<String> {
    let expanded = format!("{}/{}", summary.crates_expanded, summary.crates_total);
    let skipped = summary.crates_total - summary.crates_expanded;
    let derived = summary.report.derived_impls;
    let impls = summary.report.impls_total;
    let mut lines = vec![line(
        "crates expanded",
        &expanded,
        &format!("via rustc -Zunpretty=expanded   skipped {skipped}")
    )];
    if summary.crates_expanded > 0 {
        lines.push(line(
            "derived impls",
            &derived.to_string(),
            &format!("of {impls} impls total   (previously invisible, now measured)")
        ));
        lines.push(line("derived traits", "", &trait_tail(summary)));
    }
    lines
}

/// Formats the top derived traits by count, collapsing the long tail.
fn trait_tail(summary: &ExpandSummary) -> String {
    const SHOWN: usize = 8;
    let mut traits: Vec<(&String, &u64)> = summary.report.derived_traits.iter().collect();
    traits.sort_by(|left, right| right.1.cmp(left.1).then_with(|| left.0.cmp(right.0)));
    let shown: Vec<String> = traits
        .iter()
        .take(SHOWN)
        .map(|(name, count)| format!("{name} {count}"))
        .collect();
    let rest = traits.len().saturating_sub(SHOWN);
    if rest == 0 {
        shown.join(", ")
    } else {
        format!("{} (+{rest} more)", shown.join(", "))
    }
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
    let unresolved = report.use_targets_unresolved;
    let accounted = report.accounted_rate();
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
                "resolved in-workspace {resolved}  {resolution_rate:.1}%   external \
                 {external}   unresolved {unresolved}   accounted {accounted:.1}%"
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
    use rustmanifest_engine::{Coverage, ExpansionReport};

    use super::{ExpandSummary, expand_lines, report_lines, trait_tail};

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
            use_targets_external:       2400,
            use_targets_unresolved:     24,
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
            "use targets         4044      resolved in-workspace 1620  40.1%   external 2400   \
             unresolved 24   accounted 99.4%"
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

    fn summary(traits: &[(&str, u64)]) -> ExpandSummary {
        let mut report = ExpansionReport {
            items_expanded: 40,
            impls_total: 20,
            derived_impls: traits.iter().map(|(_, count)| count).sum(),
            ..ExpansionReport::default()
        };
        for (name, count) in traits {
            report.derived_traits.insert((*name).to_owned(), *count);
        }
        ExpandSummary {
            crates_total: 3,
            crates_expanded: 2,
            report
        }
    }

    #[test]
    fn expand_lines_report_skips_and_derived_impls() {
        let lines = expand_lines(&summary(&[("Debug", 5), ("Clone", 3)]));
        assert_eq!(
            lines[0],
            "crates expanded     2/3       via rustc -Zunpretty=expanded   skipped 1"
        );
        assert_eq!(
            lines[1],
            "derived impls       8         of 20 impls total   (previously invisible, now \
             measured)"
        );
        assert_eq!(lines[2], "derived traits                Debug 5, Clone 3");
    }

    #[test]
    fn expand_lines_stay_minimal_when_nothing_expanded() {
        let lines = expand_lines(&ExpandSummary::default());
        assert_eq!(lines.len(), 1);
        assert_eq!(
            lines[0],
            "crates expanded     0/0       via rustc -Zunpretty=expanded   skipped 0"
        );
    }

    #[test]
    fn trait_tail_sorts_by_count_and_collapses_the_rest() {
        let names: Vec<(String, u64)> = (0u32..10)
            .map(|index| (format!("Trait{index:02}"), u64::from(index) + 1))
            .collect();
        let pairs: Vec<(&str, u64)> = names
            .iter()
            .map(|(name, count)| (name.as_str(), *count))
            .collect();
        let tail = trait_tail(&summary(&pairs));
        assert!(tail.starts_with("Trait09 10, Trait08 9"));
        assert!(tail.ends_with("(+2 more)"));
    }
}
