// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The coverage section: how much of the sources the engine could read,
//! stated plainly so the reader knows what the rest of the report is
//! worth.

use std::fmt::Write as _;

use rustmanifest_engine::Workspace;

/// How much of the sources the engine could read, printed last so the
/// reader learns what the rest of the report is worth.
pub(super) fn honesty(workspace: &Workspace<'_>) -> String {
    let coverage = rustmanifest_engine::coverage(workspace);
    let mut out = format!(
        "\n## coverage\n\n- files parsed: {}/{} ({:.1}%)\n",
        coverage.files_parsed,
        coverage.files_total,
        coverage.parse_rate()
    );
    if coverage.call_candidates_total == 0 {
        out.push_str("- calls seen: 0\n");
    } else {
        let _ = writeln!(
            out,
            "- calls resolved in-workspace: {}/{}, receivers left unknown: {}",
            coverage.call_candidates_resolved,
            coverage.call_candidates_total,
            coverage.call_candidates_unknown
        );
    }
    out.push_str(&origins(&coverage, workspace));
    out.push_str(&statements(workspace));
    out.push_str("- unread sources are not a claim of absence\n");
    out
}

/// How many of the statements the engine makes about entry points hold
/// together, and how many decompose because a clause rests on a refusal.
///
/// The origin rate counts positions in the fact base; this counts what a
/// reader is actually handed. A tier can answer most positions and still
/// break every sentence, because one refused clause decomposes the whole
/// statement it stands in — which is the number a generation gate would
/// have to fire on.
fn statements(workspace: &Workspace<'_>) -> String {
    let entries = rustmanifest_engine::program_meaning(workspace);
    if entries.is_empty() {
        return String::new();
    }
    let whole = entries
        .iter()
        .filter(|entry| matches!(entry.narrative, rustmanifest_engine::Narrative::Whole(_)))
        .count();
    let total = entries.len();
    #[allow(
        clippy::cast_precision_loss,
        reason = "entry counts stay far below 2^52, so the casts are exact"
    )]
    let share = whole as f64 / total as f64 * 100.0;
    format!("- entry statements whole: {whole}/{total} ({share:.1}%)\n")
}

/// What share of the recorded value positions the engine settled, and
/// which questions stopped the rest.
///
/// Printed beside the parse and call counters because it is the one rate
/// the value tier can be read dishonestly without: every position it
/// cannot answer for is still recorded, named, and printed, so a reader
/// counting records counts refusals as understanding.
fn origins(coverage: &rustmanifest_engine::Coverage, workspace: &Workspace<'_>) -> String {
    if coverage.origins_total == 0 {
        return String::new();
    }
    let mut out = format!(
        "- value origins derived: {}/{} ({:.1}%)\n",
        coverage.origins_derived,
        coverage.origins_total,
        coverage.origin_rate()
    );
    let gaps = rustmanifest_engine::origin_gaps(workspace.facts());
    if gaps.is_empty() {
        return out;
    }
    let named = gaps
        .iter()
        .map(|(question, times)| format!("{question} {times}"))
        .collect::<Vec<_>>()
        .join(", ");
    let _ = writeln!(out, "- origins refused by question: {named}");
    out
}
