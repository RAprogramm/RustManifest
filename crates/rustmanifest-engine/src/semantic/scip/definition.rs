// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Function definitions of one indexed document and the ranges they
//! span.
//!
//! A reference occurrence is attributed to the smallest function
//! definition whose enclosing range contains it, so the layer needs the
//! definitions of a document as positioned extents. This module reads
//! them out of the occurrence list and answers the two questions the
//! attribution asks of each one: does it contain a position, and how
//! wide is it.

use super::{
    symbol::{function_symbol, qualified_name},
    wire
};

/// SCIP symbol-role bit marking a definition occurrence.
pub(super) const DEFINITION_ROLE: i32 = 1;

/// One function definition with its body extent.
pub(super) struct Definition {
    /// Qualified name parsed from the symbol.
    pub(super) name: String,
    /// Start of the enclosing range as `(line, column)`.
    start:           (i32, i32),
    /// End of the enclosing range as `(line, column)`.
    end:             (i32, i32)
}

impl Definition {
    /// Returns whether the position sits inside this definition.
    pub(super) const fn contains(&self, position: (i32, i32)) -> bool {
        let after_start = self.start.0 < position.0
            || (self.start.0 == position.0 && self.start.1 <= position.1);
        let before_end =
            position.0 < self.end.0 || (position.0 == self.end.0 && position.1 <= self.end.1);
        after_start && before_end
    }

    /// Size of the definition's extent, for smallest-enclosing choice.
    pub(super) const fn extent(&self) -> (i32, i32) {
        (self.end.0 - self.start.0, self.end.1 - self.start.1)
    }
}

/// Function definitions of one document with their enclosing ranges.
pub(super) fn function_definitions(document: &wire::Document) -> Vec<Definition> {
    let mut definitions = Vec::new();
    for occurrence in &document.occurrences {
        if occurrence.symbol_roles & DEFINITION_ROLE == 0 || !function_symbol(&occurrence.symbol) {
            continue;
        }
        let range = if occurrence.enclosing_range.is_empty() {
            &occurrence.range
        } else {
            &occurrence.enclosing_range
        };
        let Some((start, end)) = range_bounds(range) else {
            continue;
        };
        definitions.push(Definition {
            name: qualified_name(&occurrence.symbol),
            start,
            end
        });
    }
    definitions
}

/// Start position of a SCIP range, which holds three elements for a
/// single-line range and four for a multi-line one.
pub(super) fn range_start(range: &[i32]) -> Option<(i32, i32)> {
    match range.len() {
        3 | 4 => Some((range[0], range[1])),
        _ => None
    }
}

/// Start and end positions of a SCIP range.
fn range_bounds(range: &[i32]) -> Option<((i32, i32), (i32, i32))> {
    match range.len() {
        3 => Some(((range[0], range[1]), (range[0], range[2]))),
        4 => Some(((range[0], range[1]), (range[2], range[3]))),
        _ => None
    }
}
