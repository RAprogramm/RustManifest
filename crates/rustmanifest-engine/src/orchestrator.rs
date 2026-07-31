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
//!
//! After all analyzers ran on a file, the orchestrator performs an
//! unused-pragma post-pass: every inline pragma whose line index was not
//! recorded into the per-file [`Source`] usage recorder by any analyzer is
//! turned into an engine-synthesized [`UNUSED_PRAGMA_RULE_ID`] finding.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering}
    }
};

use rayon::prelude::*;
use rustmanifest_schema::{Finding, Location, Range, Severity};
use tracing::{debug, info, instrument, trace, warn};

use crate::{
    analyzer::Analyzer,
    error::{EngineError, EngineResult},
    pragma,
    source::Source
};

/// Default per-file memory budget: 10 MiB.
pub const DEFAULT_MAX_FILE_BYTES: u64 = 10 * 1024 * 1024;

/// Rule id of the engine-synthesized unused-pragma finding.
///
/// This rule does not live in any rules pack; the orchestrator emits it for
/// every inline pragma that suppressed nothing during a run.
pub const UNUSED_PRAGMA_RULE_ID: &str = "RM-META-001";

/// Rationale URI attached to unused-pragma findings.
pub const UNUSED_PRAGMA_RATIONALE_URI: &str = "rustmanifest://methodology/quick-reference";

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
        findings.extend(self.unused_pragma_findings(&source));
        debug!(path = %path.display(), findings = findings.len(), "file analyzed");
        findings
    }

    /// Emits one [`UNUSED_PRAGMA_RULE_ID`] finding per pragma in `source`
    /// that suppressed nothing during this run.
    ///
    /// Analyzers record the line index of every pragma that suppressed one
    /// of their occurrences into the source's usage recorder; a pragma left
    /// unrecorded either references a rule that produced no occurrence on
    /// the covered lines or names a rule unknown to this orchestrator's
    /// rule set. The message distinguishes the two cases.
    fn unused_pragma_findings(&self, source: &Source) -> Vec<Finding> {
        let text = source.text();
        let pragmas = pragma::collect(text);
        if pragmas.is_empty() {
            return Vec::new();
        }
        let used = source.used_pragma_lines();
        let mut findings = Vec::new();
        for unused in pragmas
            .iter()
            .filter(|item| !used.contains(&item.line_index))
        {
            let known = self
                .analyzers
                .iter()
                .any(|analyzer| analyzer.rule().id == unused.rule_id);
            let message = if known {
                format!(
                    "unused pragma: rule {} did not fire on the suppressed lines; reason was \"{}\"",
                    unused.rule_id, unused.reason
                )
            } else {
                format!(
                    "unused pragma: references unknown rule {}; reason was \"{}\"",
                    unused.rule_id, unused.reason
                )
            };
            let (start, end) = pragma::byte_span(text, unused);
            findings.push(Finding {
                rule_id: UNUSED_PRAGMA_RULE_ID.to_owned(),
                severity: Severity::Warning,
                location: Location {
                    file:  source.path().to_path_buf(),
                    range: Range {
                        start: u32::try_from(start).unwrap_or(u32::MAX),
                        end:   u32::try_from(end).unwrap_or(u32::MAX)
                    }
                },
                message,
                rationale_uri: UNUSED_PRAGMA_RATIONALE_URI.to_owned()
            });
        }
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

    fn cred_orchestrator() -> Orchestrator {
        let analyzer = PatternAnalyzer::new(cred_rule()).unwrap();
        OrchestratorBuilder::new()
            .analyzer(Box::new(analyzer))
            .build()
    }

    #[test]
    fn used_pragma_emits_no_meta_finding() {
        let dir = tempdir().unwrap();
        let file = write_file(
            dir.path(),
            "suppressed.rs",
            "// rustmanifest: allow(RM-SEC-001) reason=\"fixture\"\nlet password = \"hunter2\";\n"
        );

        let findings = cred_orchestrator().run(&[file]).unwrap();
        assert!(findings.is_empty());
    }

    #[test]
    fn unused_pragma_on_clean_line_emits_meta_finding() {
        let dir = tempdir().unwrap();
        let file = write_file(
            dir.path(),
            "clean.rs",
            "// rustmanifest: allow(RM-SEC-001) reason=\"stale suppression\"\nfn main() {}\n"
        );

        let findings = cred_orchestrator()
            .run(std::slice::from_ref(&file))
            .unwrap();
        assert_eq!(findings.len(), 1);
        let finding = &findings[0];
        assert_eq!(finding.rule_id, UNUSED_PRAGMA_RULE_ID);
        assert_eq!(finding.severity, Severity::Warning);
        assert_eq!(finding.location.file, file);
        assert_eq!(finding.location.range.start, 0);
        assert!(finding.location.range.end > finding.location.range.start);
        assert!(finding.message.contains("rule RM-SEC-001 did not fire"));
        assert!(finding.message.contains("\"stale suppression\""));
        assert_eq!(finding.rationale_uri, UNUSED_PRAGMA_RATIONALE_URI);
    }

    #[test]
    fn pragma_naming_unknown_rule_emits_unknown_variant() {
        let dir = tempdir().unwrap();
        let file = write_file(
            dir.path(),
            "typo.rs",
            "// rustmanifest: allow(RM-ZZZ-999) reason=\"typo in rule id\"\nfn main() {}\n"
        );

        let findings = cred_orchestrator().run(&[file]).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_id, UNUSED_PRAGMA_RULE_ID);
        assert!(
            findings[0]
                .message
                .contains("references unknown rule RM-ZZZ-999")
        );
        assert!(findings[0].message.contains("\"typo in rule id\""));
    }

    #[test]
    fn mixed_pragmas_yield_one_finding_for_the_unused_one() {
        let dir = tempdir().unwrap();
        let text = "// rustmanifest: allow(RM-SEC-001) reason=\"needed\"\nlet password = \
                    \"hunter2\";\n\n// rustmanifest: allow(RM-SEC-001) reason=\"stale\"\nfn main() \
                    {}\n";
        let file = write_file(dir.path(), "mixed.rs", text);

        let findings = cred_orchestrator().run(&[file]).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_id, UNUSED_PRAGMA_RULE_ID);
        assert!(findings[0].message.contains("\"stale\""));
        let expected_start = text.rfind("// rustmanifest").unwrap();
        assert_eq!(
            findings[0].location.range.start,
            u32::try_from(expected_start).unwrap()
        );
    }

    #[test]
    fn unused_pragma_findings_are_deterministic() {
        let dir = tempdir().unwrap();
        let first = write_file(
            dir.path(),
            "a.rs",
            "// rustmanifest: allow(RM-SEC-001) reason=\"stale one\"\nfn a() {}\n"
        );
        let second = write_file(
            dir.path(),
            "b.rs",
            "// rustmanifest: allow(RM-ZZZ-999) reason=\"stale two\"\nfn b() {}\n"
        );

        let orchestrator = cred_orchestrator();
        let run_one = orchestrator.run(&[second.clone(), first.clone()]).unwrap();
        let run_two = orchestrator.run(&[first.clone(), second]).unwrap();
        assert_eq!(run_one, run_two);
        assert_eq!(run_one.len(), 2);
        assert_eq!(run_one[0].location.file, first);
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
