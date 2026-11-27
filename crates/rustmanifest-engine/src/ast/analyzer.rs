// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The tier-2 analyzer itself: one rule, one built-in check, and the walk from
//! parsed file to findings through inline pragma suppression.

use rustmanifest_schema::{Finding, Location, Rule, RuleDefinition, TraceStep};
use tracing::trace;

use super::{Occurrence, check::Check};
use crate::{analyzer::Analyzer, error::EngineError, pragma, source::Source};

/// Tier-2 analyzer that applies one built-in structural check per rule.
#[derive(Debug)]
pub struct AstAnalyzer {
    rule:  Rule,
    check: Check
}

impl AstAnalyzer {
    /// Builds an analyzer for the given `ast`-tier rule.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::UnknownAstCheck`] if the rule is not an
    /// `ast`-tier rule or names a check this engine build does not provide.
    pub fn new(rule: Rule) -> Result<Self, EngineError> {
        let RuleDefinition::Ast {
            ref check
        } = rule.definition
        else {
            return Err(EngineError::UnknownAstCheck {
                rule_id: rule.id,
                check:   "<not an ast-tier rule>".to_owned()
            });
        };
        let Some(check) = Check::resolved(check) else {
            return Err(EngineError::UnknownAstCheck {
                rule_id: rule.id,
                check:   check.clone()
            });
        };
        Ok(Self {
            rule,
            check
        })
    }
}

impl Analyzer for AstAnalyzer {
    fn rule(&self) -> &Rule {
        &self.rule
    }

    fn analyze(&self, source: &Source) -> Vec<Finding> {
        let text = source.text();
        let Some(tree) = source.tree() else {
            trace!(
                rule = %self.rule.id,
                path = %source.path().display(),
                "file does not parse as Rust; skipping ast check"
            );
            return Vec::new();
        };

        let pragmas = source.pragmas();
        let mut findings = Vec::new();
        for occurrence in self.check.occurrences(tree) {
            let start = occurrence.range.start as usize;
            if let Some(active) = pragma::suppressing(text, start, &self.rule.id, pragmas) {
                source.record_pragma_use(active.line_index);
                trace!(rule = %self.rule.id, offset = start, "suppressed by pragma");
                continue;
            }
            let trace = vec![occurrence_step(&occurrence, source, &self.rule)];
            findings.push(Finding {
                rule_id: self.rule.id.clone(),
                severity: self.rule.severity,
                location: Location {
                    file:  source.path().to_path_buf(),
                    range: occurrence.range
                },
                message: format!("{}: {}", self.rule.title, occurrence.detail),
                rationale_uri: self.rule.rationale_uri.clone(),
                trace,
                fixes: Vec::new()
            });
        }
        findings
    }
}

/// Builds the single trace step of one AST occurrence: the fact cites the
/// offending item and its file, the reason reuses the check's rationale
/// sentence (the part of the detail after the first `; `, or the rule title
/// when the detail carries no rationale of its own).
fn occurrence_step(occurrence: &Occurrence, source: &Source, rule: &Rule) -> TraceStep {
    let (fact, reason) = occurrence.detail.split_once("; ").map_or_else(
        || (occurrence.detail.as_str(), rule.title.as_str()),
        |(fact, reason)| (fact, reason)
    );
    TraceStep {
        fact:   format!("{fact} in {}", source.path().display()),
        reason: reason.to_owned()
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
    use std::path::PathBuf;

    use rustmanifest_schema::Severity;

    use super::*;

    fn ast_rule(check: &str) -> Rule {
        Rule {
            id:            "RM-STRUCT-001".to_owned(),
            severity:      Severity::Warning,
            title:         "Entity naming".to_owned(),
            rationale_uri: "rustmanifest://methodology/structure#entity-naming".to_owned(),
            definition:    RuleDefinition::Ast {
                check: check.to_owned()
            }
        }
    }

    #[test]
    fn unknown_check_is_rejected() {
        let error = AstAnalyzer::new(ast_rule("no-such-check")).unwrap_err();
        assert!(matches!(error, EngineError::UnknownAstCheck { .. }));
    }

    #[test]
    fn non_ast_rule_is_rejected() {
        let mut rule = ast_rule("entity-naming");
        rule.definition = RuleDefinition::Pattern {
            regex:         "x".to_owned(),
            exclude_globs: Vec::new()
        };
        assert!(AstAnalyzer::new(rule).is_err());
    }

    #[test]
    fn unparsable_source_yields_no_findings() {
        let analyzer = AstAnalyzer::new(ast_rule("entity-naming")).unwrap();
        let source = Source::new(PathBuf::from("src/lib.rs"), "fn broken(".to_owned());
        assert_eq!(
            analyzer.analyze(&source),
            [] as [rustmanifest_schema::Finding; 0]
        );
    }

    #[test]
    fn finding_carries_rule_metadata_and_range() {
        let analyzer = AstAnalyzer::new(ast_rule("entity-naming")).unwrap();
        let source = Source::new(
            PathBuf::from("src/lib.rs"),
            "pub struct ConfigLoader;\n".to_owned()
        );
        let findings = analyzer.analyze(&source);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_id, "RM-STRUCT-001");
        assert!(findings[0].location.range.end > findings[0].location.range.start);
    }

    #[test]
    fn finding_carries_a_single_step_trace() {
        let analyzer = AstAnalyzer::new(ast_rule("entity-naming")).unwrap();
        let source = Source::new(
            PathBuf::from("src/lib.rs"),
            "pub struct ConfigLoader;\n".to_owned()
        );
        let findings = analyzer.analyze(&source);
        assert_eq!(findings.len(), 1);
        let trace = &findings[0].trace;
        assert_eq!(trace.len(), 1);
        assert_eq!(
            trace[0].fact,
            "struct `ConfigLoader` is named for what it does (`-Loader`) in src/lib.rs"
        );
        assert_eq!(trace[0].reason, "name the entity for what it is");
    }

    #[test]
    fn pragma_suppresses_ast_finding() {
        let analyzer = AstAnalyzer::new(ast_rule("entity-naming")).unwrap();
        let text = "// rustmanifest: allow(RM-STRUCT-001) reason=\"fixture\"\npub struct \
                    ConfigLoader;\n";
        let source = Source::new(PathBuf::from("src/lib.rs"), text.to_owned());
        assert_eq!(
            analyzer.analyze(&source),
            [] as [rustmanifest_schema::Finding; 0]
        );
    }
}
