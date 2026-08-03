// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The vocabulary the collection layer hands to the rule layer: one
//! loop-local `let` binding together with the byte coordinates that turn
//! it into a fix.
//!
//! These records carry facts only — where the binding sits, how it got
//! its value, how it is used. Nothing here decides whether a candidate is
//! a violation.

use rustmanifest_schema::Range;

use crate::semantic::loops::{origin::Origin, uses::UseSummary};

/// Everything needed to hoist a loop-local `let` out of its loop as a
/// byte-precise fix: where the statement's lines sit, where to insert
/// before the loop, and the indentation on both sides.
pub(in crate::semantic::loops) struct HoistPlan {
    /// Line-expanded byte range of the `let` statement, newline included.
    pub(in crate::semantic::loops) stmt_lines:   Range,
    /// Insertion offset: start of the loop's own line.
    pub(in crate::semantic::loops) insert_at:    u32,
    /// Indentation of the loop line.
    pub(in crate::semantic::loops) outer_indent: String,
    /// Indentation of the statement's line inside the loop body.
    pub(in crate::semantic::loops) inner_indent: String,
    /// Raw statement text as written.
    pub(in crate::semantic::loops) stmt_text:    String
}

/// One loop-local `let` binding eligible for either check.
pub(in crate::semantic::loops) struct Candidate {
    /// Index of the source the binding was parsed from.
    pub(in crate::semantic::loops) source:   usize,
    /// Qualified name of the enclosing function.
    pub(in crate::semantic::loops) function: String,
    /// Byte range of the binding identifier.
    pub(in crate::semantic::loops) range:    Range,
    /// Name of the binding.
    pub(in crate::semantic::loops) name:     String,
    /// How the binding obtained its value.
    pub(in crate::semantic::loops) origin:   Origin,
    /// Classified uses of the binding after its `let`.
    pub(in crate::semantic::loops) uses:     UseSummary,
    /// Hoisting coordinates when the statement sits cleanly on its own
    /// lines; `None` disables fix synthesis for the candidate.
    pub(in crate::semantic::loops) hoist:    Option<HoistPlan>
}
