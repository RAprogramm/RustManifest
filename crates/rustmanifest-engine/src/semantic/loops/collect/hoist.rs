// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Byte coordinates for lifting a `let` statement out of its loop.
//!
//! The plan is refused whenever the statement or the loop header shares a
//! line with other code, because a line-precise edit would then damage
//! that code. A refusal costs a fix, never correctness.

use rustmanifest_schema::Range;

use super::candidate::HoistPlan;

/// Builds the hoisting coordinates for a statement span, or `None` when
/// the statement does not sit cleanly on its own lines.
pub(super) fn hoist_plan(
    text: &str,
    stmt: std::ops::Range<usize>,
    loop_start: usize
) -> Option<HoistPlan> {
    let line_start = text[..stmt.start].rfind('\n').map_or(0, |at| at + 1);
    if !text[line_start..stmt.start].trim().is_empty() {
        return None;
    }
    let after = &text[stmt.end..];
    let line_break = after.find('\n')?;
    if !after[..line_break].trim().is_empty() {
        return None;
    }
    let stmt_lines_end = stmt.end + line_break + 1;
    let loop_line_start = text[..loop_start].rfind('\n').map_or(0, |at| at + 1);
    if !text[loop_line_start..loop_start].trim().is_empty() {
        return None;
    }
    Some(HoistPlan {
        stmt_lines:   Range {
            start: u32::try_from(line_start).ok()?,
            end:   u32::try_from(stmt_lines_end).ok()?
        },
        insert_at:    u32::try_from(loop_line_start).ok()?,
        outer_indent: text[loop_line_start..loop_start].to_owned(),
        inner_indent: text[line_start..stmt.start].to_owned(),
        stmt_text:    text[stmt].to_owned()
    })
}
