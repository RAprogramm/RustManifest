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
//! recorded into the per-file [`Source`](crate::source::Source) usage
//! recorder by any analyzer is turned into an engine-synthesized
//! [`UNUSED_PRAGMA_RULE_ID`] finding.

mod builder;
mod cancellation;
mod file_pass;
mod unused_pragma;

use std::{path::PathBuf, sync::Arc};

use rayon::prelude::*;
use rustmanifest_schema::Finding;
use tracing::{info, instrument};

use self::builder::CacheHandle;
pub use self::{
    builder::OrchestratorBuilder,
    cancellation::Cancellation,
    file_pass::DEFAULT_MAX_FILE_BYTES,
    unused_pragma::{UNUSED_PRAGMA_RATIONALE_URI, UNUSED_PRAGMA_RULE_ID}
};
use crate::{
    analyzer::Analyzer,
    error::{EngineError, EngineResult}
};

/// Parallel execution coordinator. Cheap to clone; the analyzer set is
/// reference-counted internally.
#[derive(Clone)]
pub struct Orchestrator {
    analyzers:      Arc<[Box<dyn Analyzer>]>,
    max_file_bytes: u64,
    cancellation:   Cancellation,
    cache:          Option<CacheHandle>
}

impl std::fmt::Debug for Orchestrator {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Orchestrator")
            .field("analyzers", &self.analyzers.len())
            .field("max_file_bytes", &self.max_file_bytes)
            .field("cancellation", &self.cancellation)
            .field("cache", &self.cache)
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
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use std::{fs, io::Write, path::Path};

    use rustmanifest_schema::{RuleDefinition, Severity};
    use tempfile::tempdir;

    use super::*;
    use crate::{cache::FindingsCache, pattern::PatternAnalyzer};

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

    fn cached_orchestrator(dir: &Path) -> Orchestrator {
        let analyzer = PatternAnalyzer::new(cred_rule()).unwrap();
        OrchestratorBuilder::new()
            .analyzer(Box::new(analyzer))
            .cache(Some(FindingsCache::open(dir).unwrap()))
            .build()
    }

    fn cache_entries(dir: &Path) -> Vec<PathBuf> {
        let mut entries: Vec<PathBuf> = fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "json")
            })
            .collect();
        entries.sort();
        entries
    }

    #[test]
    fn cache_hit_matches_fresh_run_byte_for_byte() {
        let dir = tempdir().unwrap();
        let cache_dir = tempdir().unwrap();
        let dirty = write_file(dir.path(), "dirty.rs", r#"let password = "hunter2";"#);
        let stale = write_file(
            dir.path(),
            "stale.rs",
            "// rustmanifest: allow(RM-SEC-001) reason=\"stale\"\nfn main() {}\n"
        );
        let files = [dirty, stale];

        let fresh = cred_orchestrator().run(&files).unwrap();
        let miss_run = cached_orchestrator(cache_dir.path()).run(&files).unwrap();
        let hit_run = cached_orchestrator(cache_dir.path()).run(&files).unwrap();

        let fresh_bytes = serde_json::to_vec(&fresh).unwrap();
        assert_eq!(fresh_bytes, serde_json::to_vec(&miss_run).unwrap());
        assert_eq!(fresh_bytes, serde_json::to_vec(&hit_run).unwrap());
        assert_eq!(cache_entries(cache_dir.path()).len(), 2);
    }

    #[test]
    fn cache_hit_is_served_from_the_store() {
        let dir = tempdir().unwrap();
        let cache_dir = tempdir().unwrap();
        let file = write_file(dir.path(), "dirty.rs", r#"let password = "hunter2";"#);

        cached_orchestrator(cache_dir.path())
            .run(std::slice::from_ref(&file))
            .unwrap();
        let entries = cache_entries(cache_dir.path());
        assert_eq!(entries.len(), 1);
        let tampered = fs::read_to_string(&entries[0])
            .unwrap()
            .replace("Hardcoded credentials", "tampered marker");
        fs::write(&entries[0], tampered).unwrap();

        let findings = cached_orchestrator(cache_dir.path())
            .run(std::slice::from_ref(&file))
            .unwrap();
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("tampered marker"));
    }

    #[test]
    fn changed_file_content_is_a_miss() {
        let dir = tempdir().unwrap();
        let cache_dir = tempdir().unwrap();
        let file = write_file(dir.path(), "dirty.rs", r#"let password = "hunter2";"#);

        let orchestrator = cached_orchestrator(cache_dir.path());
        let before = orchestrator.run(std::slice::from_ref(&file)).unwrap();
        assert_eq!(before.len(), 1);

        write_file(dir.path(), "dirty.rs", "fn main() {}\n");
        let after = orchestrator.run(std::slice::from_ref(&file)).unwrap();
        assert!(after.is_empty());
        assert_eq!(cache_entries(cache_dir.path()).len(), 2);
    }

    #[test]
    fn changed_rule_set_is_a_miss() {
        let dir = tempdir().unwrap();
        let cache_dir = tempdir().unwrap();
        let file = write_file(dir.path(), "dirty.rs", r#"let password = "hunter2";"#);

        cached_orchestrator(cache_dir.path())
            .run(std::slice::from_ref(&file))
            .unwrap();
        for entry in cache_entries(cache_dir.path()) {
            fs::write(entry, b"[]").unwrap();
        }

        let mut other_rule = cred_rule();
        other_rule.id = "RM-SEC-999".to_owned();
        let orchestrator = OrchestratorBuilder::new()
            .analyzer(Box::new(PatternAnalyzer::new(cred_rule()).unwrap()))
            .analyzer(Box::new(PatternAnalyzer::new(other_rule).unwrap()))
            .cache(Some(FindingsCache::open(cache_dir.path()).unwrap()))
            .build();
        let findings = orchestrator.run(std::slice::from_ref(&file)).unwrap();
        assert_eq!(findings.len(), 2);
        assert_eq!(cache_entries(cache_dir.path()).len(), 2);
    }

    #[test]
    fn corrupt_cache_entry_falls_back_to_fresh_run_and_repairs() {
        let dir = tempdir().unwrap();
        let cache_dir = tempdir().unwrap();
        let file = write_file(dir.path(), "dirty.rs", r#"let password = "hunter2";"#);

        cached_orchestrator(cache_dir.path())
            .run(std::slice::from_ref(&file))
            .unwrap();
        let entries = cache_entries(cache_dir.path());
        assert_eq!(entries.len(), 1);
        fs::write(&entries[0], b"{ definitely not a findings vec").unwrap();

        let findings = cached_orchestrator(cache_dir.path())
            .run(std::slice::from_ref(&file))
            .unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_id, "RM-SEC-001");

        let repaired = cached_orchestrator(cache_dir.path())
            .run(std::slice::from_ref(&file))
            .unwrap();
        assert_eq!(repaired, findings);
    }

    #[test]
    fn identical_files_share_an_entry_with_correct_paths() {
        let dir = tempdir().unwrap();
        let cache_dir = tempdir().unwrap();
        let text = r#"let password = "hunter2";"#;
        let first = write_file(dir.path(), "first.rs", text);
        let second = write_file(dir.path(), "second.rs", text);

        cached_orchestrator(cache_dir.path())
            .run(std::slice::from_ref(&first))
            .unwrap();
        assert_eq!(cache_entries(cache_dir.path()).len(), 1);

        let findings = cached_orchestrator(cache_dir.path())
            .run(&[first.clone(), second.clone()])
            .unwrap();
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].location.file, first);
        assert_eq!(findings[1].location.file, second);
        assert_eq!(cache_entries(cache_dir.path()).len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn unwritable_cache_dir_never_fails_the_run() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempdir().unwrap();
        let cache_dir = tempdir().unwrap();
        let file = write_file(dir.path(), "dirty.rs", r#"let password = "hunter2";"#);

        let orchestrator = cached_orchestrator(cache_dir.path());
        fs::set_permissions(cache_dir.path(), fs::Permissions::from_mode(0o555)).unwrap();
        let probe = fs::write(cache_dir.path().join("probe.json"), b"[]");
        if probe.is_ok() {
            // Permissions do not restrict this process (e.g. running as
            // root), so the unwritable-directory scenario cannot be
            // reproduced here; skip rather than assert a false premise.
            fs::set_permissions(cache_dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
            return;
        }

        let findings = orchestrator.run(std::slice::from_ref(&file)).unwrap();
        fs::set_permissions(cache_dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_id, "RM-SEC-001");
        assert!(cache_entries(cache_dir.path()).is_empty());
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
