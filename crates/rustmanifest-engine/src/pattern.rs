// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Tier-1 pattern analyzer: regex scan with glob-based file exclusion.

use std::path::Path;

use globset::{Glob, GlobSet, GlobSetBuilder};
use regex::Regex;
use rustmanifest_schema::{Finding, Location, Range, Rule, RuleDefinition};
use tracing::trace;

use crate::{analyzer::Analyzer, error::EngineError, pragma, source::Source};

/// Tier-1 analyzer that applies a single rule's regex to source text.
#[derive(Debug)]
pub struct PatternAnalyzer {
    rule:    Rule,
    regex:   Regex,
    exclude: GlobSet
}

impl PatternAnalyzer {
    /// Builds an analyzer for the given rule.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::InvalidRegex`] if the rule's regex does not
    /// compile, or [`EngineError::InvalidGlob`] if any of the rule's
    /// `exclude_globs` is not a valid glob.
    pub fn new(rule: Rule) -> Result<Self, EngineError> {
        let RuleDefinition::Pattern {
            regex: ref regex_src,
            ref exclude_globs
        } = rule.definition
        else {
            return Err(EngineError::InvalidRegex {
                rule_id: rule.id,
                regex:   String::new(),
                source:  regex::Error::Syntax("rule is not a pattern-tier rule".to_owned())
            });
        };

        let regex = Regex::new(regex_src).map_err(|source| EngineError::InvalidRegex {
            rule_id: rule.id.clone(),
            regex: regex_src.clone(),
            source
        })?;

        let mut builder = GlobSetBuilder::new();
        for glob_src in exclude_globs {
            let glob = Glob::new(glob_src).map_err(|source| EngineError::InvalidGlob {
                rule_id: rule.id.clone(),
                glob: glob_src.clone(),
                source
            })?;
            builder.add(glob);
        }
        let exclude = builder.build().map_err(|source| EngineError::InvalidGlob {
            rule_id: rule.id.clone(),
            glob: exclude_globs.join(","),
            source
        })?;

        Ok(Self {
            rule,
            regex,
            exclude
        })
    }
}

impl Analyzer for PatternAnalyzer {
    fn rule(&self) -> &Rule {
        &self.rule
    }

    fn analyze(&self, source: &Source) -> Vec<Finding> {
        let text = source.text();
        let pragmas = pragma::collect(text);

        let mut findings = Vec::new();
        for occurrence in self.regex.find_iter(text) {
            if pragma::is_suppressed(text, occurrence.start(), &self.rule.id, &pragmas) {
                trace!(
                    rule = %self.rule.id,
                    offset = occurrence.start(),
                    "suppressed by pragma"
                );
                continue;
            }
            let start = u32::try_from(occurrence.start()).unwrap_or(u32::MAX);
            let end = u32::try_from(occurrence.end()).unwrap_or(u32::MAX);
            findings.push(Finding {
                rule_id:       self.rule.id.clone(),
                severity:      self.rule.severity,
                location:      Location {
                    file:  source.path().to_path_buf(),
                    range: Range {
                        start,
                        end
                    }
                },
                message:       format!(
                    "{}: matched at bytes {}..{}",
                    self.rule.title,
                    occurrence.start(),
                    occurrence.end()
                ),
                rationale_uri: self.rule.rationale_uri.clone()
            });
        }
        findings
    }

    fn is_excluded(&self, path: &Path) -> bool {
        self.exclude.is_match(path)
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

    fn sample_rule() -> Rule {
        Rule {
            id:            "RM-SEC-001".to_owned(),
            severity:      Severity::Error,
            title:         "Hardcoded credentials".to_owned(),
            rationale_uri: "rustmanifest://methodology/security#creds".to_owned(),
            definition:    RuleDefinition::Pattern {
                regex:         r#"(?i)password\s*=\s*"[^"]+""#.to_owned(),
                exclude_globs: vec!["**/tests/**".to_owned()]
            }
        }
    }

    #[test]
    fn detects_match() {
        let analyzer = PatternAnalyzer::new(sample_rule()).unwrap();
        let source = Source::new(
            PathBuf::from("src/main.rs"),
            r#"let password = "hunter2";"#.to_owned()
        );
        let findings = analyzer.analyze(&source);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_id, "RM-SEC-001");
    }

    #[test]
    fn no_match_for_clean_source() {
        let analyzer = PatternAnalyzer::new(sample_rule()).unwrap();
        let source = Source::new(
            PathBuf::from("src/main.rs"),
            "fn validate_password(input: &str) -> bool { !input.is_empty() }".to_owned()
        );
        assert!(analyzer.analyze(&source).is_empty());
    }

    #[test]
    fn pragma_suppresses_finding() {
        let analyzer = PatternAnalyzer::new(sample_rule()).unwrap();
        let text = "// rustmanifest: allow(RM-SEC-001) reason=\"intentional fixture\"\nlet password = \
             \"hunter2\";\n";
        let source = Source::new(PathBuf::from("src/main.rs"), text.to_owned());
        assert!(analyzer.analyze(&source).is_empty());
    }

    #[test]
    fn excluded_path_is_reported_by_is_excluded() {
        let analyzer = PatternAnalyzer::new(sample_rule()).unwrap();
        assert!(analyzer.is_excluded(std::path::Path::new("src/tests/sample.rs")));
        assert!(!analyzer.is_excluded(std::path::Path::new("src/main.rs")));
    }
}
