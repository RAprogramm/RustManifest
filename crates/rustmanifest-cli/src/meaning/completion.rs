// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The status block of one entry point: the rule its signature fixes,
//! and one aligned row per site that produces a status.

use std::fmt::Write as _;

use rustmanifest_engine::{EntryMeaning, ExitClaim, completion_rule};

use super::{location::relative, wording::certainty};

/// Renders what the process leaves behind for one entry point.
pub(super) fn status_block(entry: &EntryMeaning, root: &std::path::Path) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "  status: {}", completion_rule(entry.completion));
    if entry.exits.is_empty() {
        return out;
    }
    let rows: Vec<[String; 3]> = entry.exits.iter().map(|claim| row(claim, root)).collect();
    let widths = column_widths(&rows);
    for row in &rows {
        let _ = writeln!(
            out,
            "  {:<certainty$}  {:<status$}  {}",
            row[0],
            row[1],
            row[2],
            certainty = widths[0],
            status = widths[1]
        );
    }
    out
}

/// The three columns of one row: certainty, status, where it is produced.
fn row(claim: &ExitClaim, root: &std::path::Path) -> [String; 3] {
    [
        certainty(claim.execution).to_owned(),
        claim.exit.status.to_string(),
        format!("{} ({})", claim.function, relative(&claim.file, root))
    ]
}

/// Width of every column but the last.
fn column_widths(rows: &[[String; 3]]) -> [usize; 2] {
    let mut widths = [0usize; 2];
    for row in rows {
        for (index, width) in widths.iter_mut().enumerate() {
            *width = (*width).max(row[index].chars().count());
        }
    }
    widths
}
