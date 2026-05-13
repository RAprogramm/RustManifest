// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Parallel execution coordinator for the engine.
//!
//! The orchestrator owns the rule pack (a slice of boxed [`Analyzer`]s),
//! holds tuning knobs (memory budget, cancellation token), and exposes a
//! single [`Orchestrator::run`] entry point that fans out file analysis
//! across the global rayon pool. Output is deterministic regardless of
//! scheduling: findings are sorted by `(file, byte_start, byte_end, rule_id)`
//! before being returned.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering}
    }
};

use rayon::prelude::*;
use rustmanifest_schema::Finding;
use tracing::{debug, info, instrument, trace, warn};

use crate::{
    analyzer::Analyzer,
    error::{EngineError, EngineResult},
    source::Source
};

/// Default per-file memory budget: 10 MiB.
pub const DEFAULT_MAX_FILE_BYTES: u64 = 10 * 1024 * 1024;

/// Cooperative cancellation handle threaded through the orchestrator.
#[derive(Debug, Clone, Default)]
pub struct Cancellation {
    flag: Arc<AtomicBool>
}

impl Cancellation {
    /// Creates a fresh, non-cancelled token.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Signals cancellation. Subsequent orchestrator iterations will return
    /// [`EngineError::Cancelled`].
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::Release);
    }

    /// Returns true if cancellation has been requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::Acquire)
    }
}

/// Builder for [`Orchestrator`] instances.
#[derive(Default)]
pub struct OrchestratorBuilder {
    analyzers:      Vec<Box<dyn Analyzer>>,
    max_file_bytes: Option<u64>,
    cancellation:   Option<Cancellation>
}

impl std::fmt::Debug for OrchestratorBuilder {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OrchestratorBuilder")
            .field("analyzers", &self.analyzers.len())
            .field("max_file_bytes", &self.max_file_bytes)
            .field("cancellation", &self.cancellation)
            .finish()
    }
}

impl OrchestratorBuilder {
    /// Creates an empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers one analyzer with the orchestrator.
    #[must_use]
    pub fn analyzer(mut self, analyzer: Box<dyn Analyzer>) -> Self {
        self.analyzers.push(analyzer);
        self
    }

    /// Registers a batch of analyzers, consuming the iterator.
    #[must_use]
    pub fn analyzers<I>(mut self, analyzers: I) -> Self
    where
        I: IntoIterator<Item = Box<dyn Analyzer>>
    {
        self.analyzers.extend(analyzers);
        self
    }

    /// Sets the per-file memory budget. Files larger than this are skipped.
    #[must_use]
    pub const fn max_file_bytes(mut self, bytes: u64) -> Self {
        self.max_file_bytes = Some(bytes);
        self
    }

    /// Attaches a cancellation token.
    #[must_use]
    pub fn cancellation(mut self, token: Cancellation) -> Self {
        self.cancellation = Some(token);
        self
    }

    /// Finalises the builder into an [`Orchestrator`].
    #[must_use]
    pub fn build(self) -> Orchestrator {
        Orchestrator {
            analyzers:      Arc::from(self.analyzers.into_boxed_slice()),
            max_file_bytes: self.max_file_bytes.unwrap_or(DEFAULT_MAX_FILE_BYTES),
            cancellation:   self.cancellation.unwrap_or_default()
        }
    }
}

/// Parallel execution coordinator. Cheap to clone; the analyzer set is
/// reference-counted internally.
#[derive(Clone)]
pub struct Orchestrator {
    analyzers:      Arc<[Box<dyn Analyzer>]>,
    max_file_bytes: u64,
    cancellation:   Cancellation
}

impl std::fmt::Debug for Orchestrator {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Orchestrator")
            .field("analyzers", &self.analyzers.len())
            .field("max_file_bytes", &self.max_file_bytes)
            .field("cancellation", &self.cancellation)
            .finish()
    }
}

impl Orchestrator {
    /// Returns the cancellation token attached to this orchestrator.
    #[must_use]
    pub fn cancellation(&self) -> Cancellation {
        self.cancellation.clone()
    }

    /// Returns the number of analyzers registered.
    #[must_use]
    pub fn analyzer_count(&self) -> usize {
        self.analyzers.len()
    }

    /// Runs every analyzer against every file in `files` in parallel.
    ///
    /// Returns findings sorted by `(file, byte_start, byte_end, rule_id)`.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::Cancelled`] if the cancellation token fires
    /// before the run completes. Per-file IO errors are demoted to warnings
    /// in the trace stream and the file is skipped.
    #[instrument(skip(self, files), fields(file_count = files.len(), analyzer_count = self.analyzers.len()))]
    pub fn run(&self, files: &[PathBuf]) -> EngineResult<Vec<Finding>> {
        if self.cancellation.is_cancelled() {
            return Err(EngineError::Cancelled);
        }

        let mut findings: Vec<Finding> = files
            .par_iter()
            .flat_map_iter(|path| self.analyze_one_file(path))
            .collect();

        if self.cancellation.is_cancelled() {
            return Err(EngineError::Cancelled);
        }

        findings.sort_by(|left, right| {
            left.location
                .file
                .cmp(&right.location.file)
                .then(left.location.range.start.cmp(&right.location.range.start))
                .then(left.location.range.end.cmp(&right.location.range.end))
                .then(left.rule_id.cmp(&right.rule_id))
        });

        info!(findings = findings.len(), "analysis complete");
        Ok(findings)
    }

    fn analyze_one_file(&self, path: &Path) -> Vec<Finding> {
        if self.cancellation.is_cancelled() {
            return Vec::new();
        }

        let metadata = match fs::metadata(path) {
            Ok(metadata) => metadata,
            Err(err) => {
                warn!(path = %path.display(), error = %err, "metadata read failed");
                return Vec::new();
            }
        };
        if metadata.len() > self.max_file_bytes {
            warn!(
                path = %path.display(),
                bytes = metadata.len(),
                limit = self.max_file_bytes,
                "file exceeds memory budget; skipped"
            );
            return Vec::new();
        }

        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) => {
                warn!(path = %path.display(), error = %err, "read failed; skipped");
                return Vec::new();
            }
        };

        let source = Source::new(path.to_path_buf(), text);
        let mut findings = Vec::new();
        for analyzer in self.analyzers.as_ref() {
            if analyzer.is_excluded(path) {
                trace!(
                    rule = %analyzer.rule().id,
                    path = %path.display(),
                    "excluded by rule glob"
                );
                continue;
            }
            findings.extend(analyzer.analyze(&source));
        }
        debug!(path = %path.display(), findings = findings.len(), "file analyzed");
        findings
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use std::io::Write;

    use rustmanifest_schema::{RuleDefinition, Severity};
    use tempfile::tempdir;

    use super::*;
    use crate::pattern::PatternAnalyzer;

    fn write_file(dir: &Path, name: &str, contents: &str) -> PathBuf {
        let path = dir.join(name);
        let mut handle = fs::File::create(&path).unwrap();
        handle.write_all(contents.as_bytes()).unwrap();
        path
    }

    fn cred_rule() -> rustmanifest_schema::Rule {
        rustmanifest_schema::Rule {
            id:            "RM-SEC-001".to_owned(),
            severity:      Severity::Error,
            title:         "Hardcoded credentials".to_owned(),
            rationale_uri: "rustmanifest://methodology/security#creds".to_owned(),
            definition:    RuleDefinition::Pattern {
                regex:         r#"(?i)password\s*=\s*"[^"]+""#.to_owned(),
                exclude_globs: vec![]
            }
        }
    }

    #[test]
    fn runs_on_files_and_returns_findings() {
        let dir = tempdir().unwrap();
        let bad = write_file(dir.path(), "bad.rs", r#"let password = "hunter2";"#);
        let good = write_file(dir.path(), "good.rs", "fn main() {}");

        let analyzer = PatternAnalyzer::new(cred_rule()).unwrap();
        let orchestrator = OrchestratorBuilder::new()
            .analyzer(Box::new(analyzer))
            .build();

        let findings = orchestrator.run(&[bad.clone(), good]).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].location.file, bad);
    }

    #[test]
    fn cancellation_short_circuits() {
        let token = Cancellation::new();
        token.cancel();
        let analyzer = PatternAnalyzer::new(cred_rule()).unwrap();
        let orchestrator = OrchestratorBuilder::new()
            .analyzer(Box::new(analyzer))
            .cancellation(token)
            .build();

        let err = orchestrator
            .run(&[PathBuf::from("does/not/matter.rs")])
            .unwrap_err();
        assert!(matches!(err, EngineError::Cancelled));
    }

    #[test]
    fn oversized_files_are_skipped() {
        let dir = tempdir().unwrap();
        let huge = write_file(
            dir.path(),
            "huge.rs",
            &"let password = \"x\";\n".repeat(10_000)
        );

        let analyzer = PatternAnalyzer::new(cred_rule()).unwrap();
        let orchestrator = OrchestratorBuilder::new()
            .analyzer(Box::new(analyzer))
            .max_file_bytes(10)
            .build();

        let findings = orchestrator.run(&[huge]).unwrap();
        assert!(findings.is_empty());
    }
}
