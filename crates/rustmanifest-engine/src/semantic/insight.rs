// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The unit every semantic check reports: a workspace-level violation
//! with its anchoring location and derivation trace.
//!
//! Lives in its own module so that check modules depend on it directly
//! instead of reaching back into the tier root — keeping the semantic
//! tier's own module graph acyclic.

use rustmanifest_schema::{Fix, TraceStep};

/// One workspace-level violation reported by a semantic check.
#[derive(Debug)]
pub(super) struct Insight {
    /// Index of the source the finding anchors to.
    pub(super) source: usize,
    /// Byte range of the anchoring definition.
    pub(super) range:  rustmanifest_schema::Range,
    /// Check-specific description including the reasoning chain.
    pub(super) detail: String,
    /// Machine-readable derivation steps backing the detail.
    pub(super) trace:  Vec<TraceStep>,
    /// Byte-precise fixes resolving the violation, when provable.
    pub(super) fixes:  Vec<Fix>
}

impl Insight {
    /// Creates an insight anchored to `source` at `range`.
    pub(super) const fn new(
        source: usize,
        range: rustmanifest_schema::Range,
        detail: String,
        trace: Vec<TraceStep>
    ) -> Self {
        Self {
            source,
            range,
            detail,
            trace,
            fixes: Vec::new()
        }
    }

    /// Attaches provable fixes to the insight.
    pub(super) fn with_fixes(mut self, fixes: Vec<Fix>) -> Self {
        self.fixes = fixes;
        self
    }
}
