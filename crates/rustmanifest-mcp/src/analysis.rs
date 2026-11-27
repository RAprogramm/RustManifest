// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Engine composition mirroring the CLI `check` pipeline.
//!
//! Pattern and AST tiers run per file through the [`Orchestrator`]; the
//! semantic tier is evaluated once over every discovered Rust source. The
//! target may be a single file or a directory, in which case files are
//! discovered through the gitignore-aware [`walker`].

use std::path::{Path, PathBuf};

use rustmanifest_engine::{
    Analyzer, AstAnalyzer, EngineError, OrchestratorBuilder, PatternAnalyzer, SemanticAnalyzer,
    Source, orchestrator::DEFAULT_MAX_FILE_BYTES, walker
};
use rustmanifest_schema::{Finding, Rule, RuleDefinition};

/// Runs the given rules against `target` (a file or a directory).
///
/// Findings from the per-file tiers arrive sorted by
/// `(file, byte_start, byte_end, rule_id)`; semantic findings are appended
/// after them, matching the CLI composition.
///
/// # Errors
///
/// Returns an [`EngineError`] when an analyzer cannot be built from a rule,
/// when file discovery fails, or when the orchestrator run fails.
pub fn run_rules<'rules, I>(rules: I, target: &Path) -> Result<Vec<Finding>, EngineError>
where
    I: IntoIterator<Item = &'rules Rule>
{
    let mut per_file: Vec<Box<dyn Analyzer>> = Vec::new();
    let mut semantic: Vec<SemanticAnalyzer> = Vec::new();
    for rule in rules {
        match rule.definition {
            RuleDefinition::Pattern {
                ..
            } => {
                per_file.push(Box::new(PatternAnalyzer::new(rule.clone())?));
            }
            RuleDefinition::Ast {
                ..
            } => per_file.push(Box::new(AstAnalyzer::new(rule.clone())?)),
            RuleDefinition::Semantic {
                ..
            } => semantic.push(SemanticAnalyzer::new(rule.clone())?)
        }
    }

    let files = discover_targets(target)?;
    let orchestrator = OrchestratorBuilder::new().analyzers(per_file).build();
    let mut findings = orchestrator.run(&files)?;
    findings.extend(semantic_findings(&semantic, &files));
    Ok(findings)
}

/// Expands `target` into the list of files to analyze.
///
/// A file is analyzed as-is; a directory is walked for Rust sources.
fn discover_targets(target: &Path) -> Result<Vec<PathBuf>, EngineError> {
    if target.is_dir() {
        walker::discover(&[target])
    } else {
        Ok(vec![target.to_path_buf()])
    }
}

/// Evaluates the workspace-level semantic tier over every readable Rust
/// source within the memory budget and returns its findings.
fn semantic_findings(analyzers: &[SemanticAnalyzer], files: &[PathBuf]) -> Vec<Finding> {
    if analyzers.is_empty() {
        return Vec::new();
    }
    let sources: Vec<Source> = files
        .iter()
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .filter(|path| {
            std::fs::metadata(path).is_ok_and(|metadata| metadata.len() <= DEFAULT_MAX_FILE_BYTES)
        })
        .filter_map(|path| {
            std::fs::read_to_string(path)
                .ok()
                .map(|text| Source::new(path.clone(), text))
        })
        .collect();
    rustmanifest_engine::semantic_findings(
        analyzers,
        &rustmanifest_engine::Workspace::new(&sources)
    )
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use std::fs;

    use rustmanifest_rules_core::RULES;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn single_file_target_reports_pattern_finding() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("bad.rs");
        fs::write(&file, "fn main() { let password = \"hunter2\"; }\n").unwrap();
        let findings = run_rules(RULES.iter(), &file).unwrap();
        assert!(
            findings
                .iter()
                .any(|finding| finding.rule_id == "RM-SEC-001")
        );
    }

    #[test]
    fn directory_target_discovers_and_reports() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("naming.rs"), "pub struct ConfigLoader;\n").unwrap();
        let findings = run_rules(RULES.iter(), dir.path()).unwrap();
        assert!(
            findings
                .iter()
                .any(|finding| finding.rule_id == "RM-STRUCT-001")
        );
    }

    #[test]
    fn clean_file_yields_no_findings() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("clean.rs");
        fs::write(&file, "fn main() {}\n").unwrap();
        let findings = run_rules(RULES.iter(), &file).unwrap();
        assert_eq!(findings, [] as [rustmanifest_schema::Finding; 0]);
    }
}
