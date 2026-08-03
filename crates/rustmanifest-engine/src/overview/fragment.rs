// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The two writing primitives every overview section shares: appending a
//! fragment to the report, and announcing that a ranked list was cut short.
//!
//! Truncation is never silent, so the note lives next to the append it is
//! built on rather than being restated in each section.

/// Appends a note when a ranked list was truncated.
pub(super) fn truncation_note(out: &mut String, total: usize, top: usize) {
    if total > top {
        push(out, &format!("- ... and {} more\n", total - top));
    }
}

/// Appends a fragment to the report.
pub(super) fn push(out: &mut String, fragment: &str) {
    out.push_str(fragment);
}
