// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Common analyzer trait shared by every tier.

use std::path::Path;

use rustmanifest_schema::{Finding, Rule};

use crate::source::ParsedSource;

/// Trait implemented by every analyzer in the engine, regardless of tier.
///
/// Implementations MUST be deterministic: the same file produces the same
/// findings in the same order on every invocation.
pub trait Analyzer: Send + Sync {
    /// Returns the rule this analyzer evaluates.
    fn rule(&self) -> &Rule;

    /// Analyzes a single source file and returns all findings produced.
    ///
    /// A tier that reasons over text reads [`ParsedSource::source`]; a tier
    /// that reasons over syntax reads [`ParsedSource::tree`], which parses
    /// the file at most once however many analyzers ask for it.
    fn analyze(&self, file: &ParsedSource<'_>) -> Vec<Finding>;

    /// Returns true if the rule's configuration excludes this path from
    /// analysis (e.g. it matches one of the rule's `exclude_globs`).
    ///
    /// The default returns `false`. The orchestrator calls this before
    /// reading file contents, so excluded files are never loaded into
    /// memory.
    fn is_excluded(&self, _path: &Path) -> bool {
        false
    }
}
