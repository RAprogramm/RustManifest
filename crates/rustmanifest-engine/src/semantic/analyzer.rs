// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The tier-3 analyzer itself: one rule, one built-in check, and the walk
//! from a workspace derivation to findings through inline pragma
//! suppression.
//!
//! Lives beside the tier root so that the root carries only module
//! declarations and re-exports, and the analyzer carries only the decision
//! it makes.

use rustmanifest_schema::{Finding, Location, Rule, RuleDefinition};
use tracing::trace;

use super::{check::Check, workspace::Workspace};
use crate::{error::EngineError, pragma};

/// Tier-3 analyzer evaluating one semantic check over the whole workspace.
#[derive(Debug)]
pub struct SemanticAnalyzer {
    rule:  Rule,
    check: Check
}

impl SemanticAnalyzer {
    /// Builds an analyzer for the given `semantic`-tier rule.
    ///
    /// # Errors
    ///
    /// Returns an [`AppError`] if the rule is not a `semantic`-tier rule or
    /// names a check this engine build does not provide.
    pub fn new(rule: Rule) -> Result<Self, masterror::AppError> {
        let RuleDefinition::Semantic {
            ref check
        } = rule.definition
        else {
            return Err(EngineError::UnknownAstCheck {
                rule_id: rule.id,
                check:   "<not a semantic-tier rule>".to_owned()
            }
            .into());
        };
        let Some(check) = Check::resolved(check) else {
            return Err(EngineError::UnknownAstCheck {
                rule_id: rule.id,
                check:   check.clone()
            }
            .into());
        };
        Ok(Self {
            rule,
            check
        })
    }

    /// Evaluates the check over `workspace`, honoring pragma suppression in
    /// the file each finding anchors to.
    ///
    /// The workspace derivations arrive as an argument rather than being
    /// taken here, so that a whole rule set derives each of them once
    /// instead of once per rule. [`semantic_findings`] is that composition
    /// for the common case.
    pub fn analyze(&self, workspace: &Workspace<'_>) -> Vec<Finding> {
        let mut findings = Vec::new();
        for insight in self.check.insights(workspace) {
            let Some(source) = workspace.sources().get(insight.source) else {
                continue;
            };
            let text = source.text();
            let pragmas = source.pragmas();
            let start = insight.range.start as usize;
            if let Some(active) = pragma::suppressing(text, start, &self.rule.id, pragmas) {
                source.record_pragma_use(active.line_index);
                trace!(rule = %self.rule.id, offset = start, "suppressed by pragma");
                continue;
            }
            findings.push(Finding {
                rule_id:       self.rule.id.clone(),
                severity:      self.rule.severity,
                location:      Location {
                    file:  source.path().to_path_buf(),
                    range: insight.range
                },
                message:       format!("{}: {}", self.rule.title, insight.detail),
                rationale_uri: self.rule.rationale_uri.clone(),
                trace:         insight.trace,
                fixes:         insight.fixes
            });
        }
        findings
    }
}

/// Runs every analyzer in `analyzers` over `sources` and returns their
/// findings in analyzer order.
///
/// The workspace derivations — type graph, behavioral facts, module graph —
/// are taken once and shared by all of them: each is a property of the
/// sources, not of the rule asking about them, and deriving them per rule
/// made the tier's cost scale with the size of the rule set.
#[must_use]
pub fn semantic_findings(
    analyzers: &[SemanticAnalyzer],
    workspace: &Workspace<'_>
) -> Vec<Finding> {
    analyzers
        .iter()
        .flat_map(|analyzer| analyzer.analyze(workspace))
        .collect()
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use std::path::PathBuf;

    use rustmanifest_schema::Severity;

    use super::*;
    use crate::{semantic::workspace::Workspace, source::Source};

    fn semantic_rule(check: &str) -> Rule {
        Rule {
            id:            "RM-STRUCT-010".to_owned(),
            severity:      Severity::Warning,
            title:         "Repeated field group".to_owned(),
            rationale_uri: "rustmanifest://methodology/structure#structure-size".to_owned(),
            definition:    RuleDefinition::Semantic {
                check: check.to_owned()
            }
        }
    }

    fn source(name: &str, text: &str) -> Source {
        Source::new(PathBuf::from(name), text.to_owned())
    }

    #[test]
    fn unknown_check_is_rejected() {
        let error = SemanticAnalyzer::new(semantic_rule("no-such-check")).unwrap_err();
        assert!(error.to_string().contains("unknown ast check"));
    }

    #[test]
    fn repeated_group_across_files_is_reported() {
        let analyzer = SemanticAnalyzer::new(semantic_rule("repeated-field-group")).unwrap();
        let sources = [
            source(
                "src/user.rs",
                "struct User { id: u64, created_at: u64, updated_at: u64 }"
            ),
            source(
                "src/post.rs",
                "struct Post { text: String, created_at: u64, updated_at: u64 }"
            )
        ];
        let findings = analyzer.analyze(&Workspace::resolved(
            &sources,
            &crate::semantic::settling::settled_over(&sources)
        ));
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("created_at"));
        assert!(findings[0].message.contains("User"));
        assert!(findings[0].message.contains("Post"));
        assert_eq!(findings[0].trace.len(), 2);
        assert!(findings[0].trace[0].fact.contains("struct `User`"));
        assert!(findings[0].trace[0].fact.contains("src/user.rs"));
        assert_eq!(findings[0].trace[0].reason, "shares the group");
    }

    #[test]
    fn pragma_in_anchor_file_suppresses() {
        let analyzer = SemanticAnalyzer::new(semantic_rule("repeated-field-group")).unwrap();
        let sources = [
            source(
                "src/user.rs",
                "// rustmanifest: allow(RM-STRUCT-010) reason=\"fixture\"\nstruct User { id: \
                 u64, created_at: u64, updated_at: u64 }"
            ),
            source(
                "src/post.rs",
                "struct Post { text: String, created_at: u64, updated_at: u64 }"
            )
        ];
        assert_eq!(
            analyzer.analyze(&Workspace::resolved(
                &sources,
                &crate::semantic::settling::settled_over(&sources)
            )),
            [] as [rustmanifest_schema::Finding; 0]
        );
    }
}
