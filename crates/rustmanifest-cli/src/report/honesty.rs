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
        let _ = writeln!(
            out,
            "- refused calls by row: foreign {}, named-elsewhere {}, ambiguous {}, \
             owner-without-member {}, granted-without-body {}, construction {}",
            coverage.call_candidates_external,
            coverage.call_candidates_unreachable,
            coverage.call_candidates_ambiguous,
            coverage.call_candidates_owned,
            coverage.call_candidates_granted,
            coverage.call_candidates_built
        );
    }
    out.push_str(&origins(&coverage, workspace));
    out.push_str(&statements(workspace));
    out.push_str(&database_statements(workspace));
    out.push_str("- unread sources are not a claim of absence\n");
    out
}

/// What share of the database effects the statement channel accounts
/// for, and how many of the accounted statements name their table.
///
/// The gap between the first two numbers is the channel's blind spot —
/// statements built at run time, DDL — and printing it keeps a table
/// sentence from reading as if it covered every query the program
/// makes.
fn database_statements(workspace: &Workspace<'_>) -> String {
    let counted = rustmanifest_engine::semantic::statement_coverage(workspace.facts());
    if counted.effect_sites == 0 {
        return String::new();
    }
    format!(
        "- database statements with a proven kind: {}/{}, naming their table: {}/{}\n",
        counted.kinds_proven, counted.effect_sites, counted.tables_named, counted.kinds_proven
    )
}

/// How many of the statements the engine makes about entry points hold
/// together, how many decompose because a clause rests on a refusal, and
/// how many of the positions a program actually prints have their value
/// traced all the way.
///
/// The last of the three is the strict one. A statement holds together
/// whenever no clause would have to speak a refusal, which most programs
/// manage; what a generator would need is the value behind each written
/// position, and that is counted separately so the easier number cannot
/// stand in for it.
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
    let positions: Vec<bool> = entries
        .iter()
        .flat_map(|entry| entry.claims.iter())
        .flat_map(|claim| claim.sources.iter())
        .map(|(_, origin)| origin.derived())
        .collect();
    let filled = positions.iter().filter(|derived| **derived).count();
    #[allow(
        clippy::cast_precision_loss,
        reason = "entry counts stay far below 2^52, so the casts are exact"
    )]
    let share = whole as f64 / total as f64 * 100.0;
    let mut out = format!("- entry statements whole: {whole}/{total} ({share:.1}%)\n");
    if !positions.is_empty() {
        #[allow(
            clippy::cast_precision_loss,
            reason = "position counts stay far below 2^52, so the casts are exact"
        )]
        let told = filled as f64 / positions.len() as f64 * 100.0;
        let _ = writeln!(
            out,
            "- written positions whose value is traced: {filled}/{} ({told:.1}%)",
            positions.len()
        );
    }
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
