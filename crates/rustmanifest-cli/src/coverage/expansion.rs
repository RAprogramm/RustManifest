// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The `--expand` block: what derives actually generate, and what the type
//! graph gains once those expansions are absorbed.

use super::layout::line;
use crate::expand::ExpandSummary;

/// Renders the `--expand` report block appended after the coverage lines.
pub(super) fn expand_lines(summary: &ExpandSummary) -> Vec<String> {
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

/// Counts the drift between the two universes of analysis: functions
/// both see, functions only the plain reading sees (bodies behind
/// `cfg(test)` and friends, stripped by the expansion build), and
/// functions only the expansion sees (macro-generated bodies absent
/// from surface syntax). The two universes answer different questions
/// by definition of the mode; the drift makes their difference a
/// number instead of an invisibility.
pub(super) fn universe_drift(
    plain: &std::collections::BTreeSet<String>,
    expanded: &std::collections::BTreeSet<String>
) -> (usize, usize, usize) {
    let shared = plain.intersection(expanded).count();
    let plain_only = plain.difference(expanded).count();
    let expanded_only = expanded.difference(plain).count();
    (shared, plain_only, expanded_only)
}

/// Renders the universe-drift line from the plain facts and the
/// expanded streams: each stream is read with the same reading the
/// plain universe got, so the two sides are comparable by name.
pub(super) fn drift_line(
    sources: &[rustmanifest_engine::Source],
    summary: &ExpandSummary
) -> String {
    let plain: std::collections::BTreeSet<String> = rustmanifest_engine::behavior(sources)
        .into_iter()
        .map(|fact| fact.function)
        .collect();
    let mut expanded: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for (path, stream) in &summary.streams {
        let synthetic = rustmanifest_engine::Source::new(path.clone(), stream.clone());
        expanded.extend(
            rustmanifest_engine::behavior(std::slice::from_ref(&synthetic))
                .into_iter()
                .map(|fact| fact.function)
        );
    }
    let (shared, plain_only, expanded_only) = universe_drift(&plain, &expanded);
    line(
        "universe drift",
        &shared.to_string(),
        &format!(
            "shared   plain-only {plain_only} (cfg-stripped bodies)   expanded-only \
             {expanded_only} (macro-generated)"
        )
    )
}

/// Absorbs every expanded stream into a fresh type graph and renders the
/// integration line: how many derived impls the graph now holds and how
/// many workspace types gained known trait implementations.
pub(super) fn integration_line(
    sources: &[rustmanifest_engine::Source],
    summary: &ExpandSummary
) -> String {
    let mut graph = rustmanifest_engine::TypeGraph::from_sources(sources);
    let mut impls = 0;
    let mut types = 0;
    for (_, stream) in &summary.streams {
        let absorbed = graph.absorb_expansion(stream);
        impls += absorbed.impls;
        types += absorbed.types;
    }
    let covered = graph
        .types()
        .iter()
        .filter(|node| {
            graph
                .impls()
                .iter()
                .any(|impl_block| impl_block.derived && impl_block.type_name == node.name)
        })
        .count();
    line(
        "graph integration",
        &format!("+{impls}"),
        &format!(
            "derived impls absorbed   generated types discovered {types}   types with known \
             derives {covered}"
        )
    )
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

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    /// The drift is three one-sided counts, none derivable from the
    /// others by subtraction alone.
    #[test]
    fn the_drift_counts_each_side_by_name() {
        use std::collections::BTreeSet;
        let plain: BTreeSet<String> = ["shared", "test_only"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        let expanded: BTreeSet<String> = ["shared", "generated"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        assert_eq!(super::universe_drift(&plain, &expanded), (1, 1, 1));
        assert_eq!(super::universe_drift(&plain, &plain), (2, 0, 0));
    }

    /// The drift line keeps its columns whatever the counts measure: a
    /// four-digit shared count is a value like any other, and the words
    /// naming it live in the tail, so the separator cannot be eaten.
    #[test]
    fn the_drift_line_keeps_its_columns_at_four_digits() {
        let rendered = super::line("universe drift", "1395", "shared   plain-only 1234");
        assert!(
            rendered.starts_with("universe drift      1395      shared"),
            "{rendered}"
        );
    }

    use rustmanifest_engine::ExpansionReport;

    use super::{ExpandSummary, expand_lines, trait_tail};

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
            report,
            streams: Vec::new()
        }
    }

    #[test]
    fn integration_line_reports_absorbed_impls_and_covered_types() {
        let sources = [rustmanifest_engine::Source::new(
            std::path::PathBuf::from("src/lib.rs"),
            "pub struct User { pub id: u64 }".to_owned()
        )];
        let mut summary = summary(&[]);
        summary.streams = vec![(
            std::path::PathBuf::from("."),
            "#[automatically_derived]\nimpl ::core::fmt::Debug for User { }".to_owned()
        )];
        assert_eq!(
            super::integration_line(&sources, &summary),
            "graph integration   +1        derived impls absorbed   generated types discovered \
             0   types with known derives 1"
        );
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
