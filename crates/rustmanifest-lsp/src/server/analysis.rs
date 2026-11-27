// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! One document text to its diagnostics.
//!
//! The same bundled rules the CLI runs, over the text the editor holds
//! rather than the bytes on disk — an unsaved buffer is the program the
//! author is asking about. The file-scoped tiers and the semantic tier
//! both run; workspace-wide invariants do not, because one open buffer
//! is not the workspace they are declared over, and firing them here
//! would state of one file what was declared of many.

use std::path::PathBuf;

use lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString};
use rustmanifest_engine::{Analyzer, AstAnalyzer, PatternAnalyzer, SemanticAnalyzer, Source};
use rustmanifest_schema::{Rule, RuleDefinition, Severity};

use super::position::LineIndex;

/// The analyzers built from the bundled rules, once per server.
pub(super) struct Analysis {
    tiered:   Vec<Box<dyn Analyzer>>,
    semantic: Vec<SemanticAnalyzer>
}

impl Analysis {
    /// Builds every analyzer the bundled rules define.
    ///
    /// # Errors
    ///
    /// Returns the first rule whose analyzer failed to build.
    pub(super) fn new() -> masterror::AppResult<Self> {
        let rules: Vec<&Rule> = rustmanifest_rules_core::RULES.iter().collect();
        let mut tiered: Vec<Box<dyn Analyzer>> = Vec::new();
        let mut semantic = Vec::new();
        for rule in rules {
            match rule.definition {
                RuleDefinition::Pattern {
                    ..
                } => tiered.push(Box::new(PatternAnalyzer::new(rule.clone())?)),
                RuleDefinition::Ast {
                    ..
                } => tiered.push(Box::new(AstAnalyzer::new(rule.clone())?)),
                RuleDefinition::Semantic {
                    ..
                } => semantic.push(SemanticAnalyzer::new(rule.clone())?)
            }
        }
        Ok(Self {
            tiered,
            semantic
        })
    }

    /// The findings of one document text, fixes included.
    ///
    /// The semantic tier answers from what a compiler settled, so the
    /// resolution the session holds is handed to it: without one every
    /// such reading refuses, and the editor would show a quieter file
    /// than the same run on the command line.
    pub(super) fn findings(
        &self,
        path: PathBuf,
        text: &str,
        settled: Option<&rustmanifest_schema::Resolution>
    ) -> Vec<rustmanifest_schema::Finding> {
        let source = Source::new(path, text.to_owned());
        let mut findings: Vec<rustmanifest_schema::Finding> = self
            .tiered
            .iter()
            .filter(|analyzer| !analyzer.is_excluded(source.path()))
            .flat_map(|analyzer| analyzer.analyze(&source))
            .collect();
        let held = [source];
        let workspace = settled.map_or_else(
            || rustmanifest_engine::Workspace::new(&held),
            |settled| rustmanifest_engine::Workspace::resolved(&held, settled)
        );
        findings.extend(rustmanifest_engine::semantic_findings(
            &self.semantic,
            &workspace
        ));
        findings
    }

    /// The diagnostics of one document text.
    pub(super) fn diagnostics(
        &self,
        path: PathBuf,
        text: &str,
        settled: Option<&rustmanifest_schema::Resolution>
    ) -> Vec<Diagnostic> {
        let index = LineIndex::new(text);
        self.findings(path, text, settled)
            .into_iter()
            .map(|finding| Diagnostic {
                range: lsp_types::Range {
                    start: index.position(text, finding.location.range.start as usize),
                    end:   index.position(text, finding.location.range.end as usize)
                },
                severity: Some(severity(finding.severity)),
                code: Some(NumberOrString::String(finding.rule_id)),
                source: Some("rust-manifest".to_owned()),
                message: finding.message,
                ..Diagnostic::default()
            })
            .collect()
    }
}

/// The protocol severity one finding severity maps to.
const fn severity(graded: Severity) -> DiagnosticSeverity {
    match graded {
        Severity::Error => DiagnosticSeverity::ERROR,
        Severity::Warning => DiagnosticSeverity::WARNING,
        Severity::Info => DiagnosticSeverity::INFORMATION,
        Severity::Hint => DiagnosticSeverity::HINT
    }
}
