// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Tiered analysis engine for `rustmanifest`.
//!
//! The engine exposes three tiers selectable per-rule:
//!
//! - **Pattern** — fast regex / aho-corasick scan over file text.
//! - **AST** — `syn`-based traversal for structural and local semantic checks.
//! - **Semantic** — full semantic analysis via `rust-analyzer` or cargo
//!   integrations (off by default).
//!
//! Phase 0 ships only the public trait surface; concrete implementations and
//! rule wiring land in Phase 1.

use rustmanifest_schema::{Finding, Rule};

/// Trait implemented by every analyzer in the engine, regardless of tier.
pub trait Analyzer {
    /// Returns the rule this analyzer evaluates.
    fn rule(&self) -> &Rule;

    /// Analyzes a single source file and returns all findings produced.
    ///
    /// Implementations MUST be deterministic: same input bytes and same rule
    /// produce the same findings in the same order.
    fn analyze(&self, source: &str) -> Vec<Finding>;
}
