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
    out.push_str("- unread sources are not a claim of absence\n");
    out
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
