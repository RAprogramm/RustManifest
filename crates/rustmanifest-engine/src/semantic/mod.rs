// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Tier-3 semantic analysis: workspace-level reasoning over the type graph.
//!
//! Unlike tiers 1 and 2, which inspect one file at a time, this tier builds
//! a [`TypeGraph`] across every analyzed source and evaluates checks that
//! need relationships between definitions: which structure aggregates which,
//! which field groups repeat, what a field's type refers to. Checks that
//! reason about modules rather than types build a module graph from the
//! same source slice — crate roots, `mod` resolution, and per-module `use`
//! targets.

mod composition;
mod coverage;
mod expansion;
mod field_group;
mod graph;
mod module_cycle;
mod module_graph;
mod module_overview;

use rustmanifest_schema::{Finding, Location, Rule, RuleDefinition, TraceStep};
use tracing::trace;

pub use self::{
    coverage::{Coverage, coverage, coverage_with, percent, unresolved_targets},
    expansion::{ExpansionReport, expansion},
    graph::{FieldNode, ImplNode, MethodNode, TraitNode, TypeGraph, TypeKind, TypeNode},
    module_overview::{ModuleEdge, ModuleInfo, ModuleOverview, module_overview}
};
use crate::{error::EngineError, pragma, source::Source};

/// One workspace-level violation reported by a semantic check.
#[derive(Debug)]
pub(crate) struct Insight {
    /// Index of the source the finding anchors to.
    source: usize,
    /// Byte range of the anchoring definition.
    range:  rustmanifest_schema::Range,
    /// Check-specific description including the reasoning chain.
    detail: String,
    /// Machine-readable derivation steps backing the detail.
    trace:  Vec<TraceStep>
}

impl Insight {
    /// Creates an insight anchored to `source` at `range`.
    pub(crate) const fn new(
        source: usize,
        range: rustmanifest_schema::Range,
        detail: String,
        trace: Vec<TraceStep>
    ) -> Self {
        Self {
            source,
            range,
            detail,
            trace
        }
    }
}

/// Built-in semantic checks addressable from `definition.semantic`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Check {
    /// `repeated-field-group` — identical field groups across structures.
    RepeatedFieldGroup,
    /// `module-cycles` — cycles in the module dependency graph.
    ModuleCycles,
    /// `structure-size` — oversized structures with a graph-derived split
    /// suggestion.
    StructureSize
}

impl Check {
    /// Resolves a check identifier from the rules pack.
    fn resolved(name: &str) -> Option<Self> {
        match name {
            "repeated-field-group" => Some(Self::RepeatedFieldGroup),
            "module-cycles" => Some(Self::ModuleCycles),
            "structure-size" => Some(Self::StructureSize),
            _ => None
        }
    }

    /// Runs the check over the workspace sources and their type graph.
    fn insights(self, sources: &[Source], graph: &TypeGraph) -> Vec<Insight> {
        match self {
            Self::RepeatedFieldGroup => field_group::insights(sources, graph),
            Self::ModuleCycles => module_cycle::insights(sources),
            Self::StructureSize => composition::insights(graph)
        }
    }
}

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
    /// Returns [`EngineError::UnknownAstCheck`] if the rule is not a
    /// `semantic`-tier rule or names a check this engine build does not
    /// provide.
    pub fn new(rule: Rule) -> Result<Self, EngineError> {
        let RuleDefinition::Semantic {
            ref check
        } = rule.definition
        else {
            return Err(EngineError::UnknownAstCheck {
                rule_id: rule.id,
                check:   "<not a semantic-tier rule>".to_owned()
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

    /// Evaluates the check over `sources`, honoring pragma suppression in
    /// the file each finding anchors to.
    pub fn analyze(&self, sources: &[Source]) -> Vec<Finding> {
        let graph = TypeGraph::from_sources(sources);
        let mut findings = Vec::new();
        for insight in self.check.insights(sources, &graph) {
            let Some(source) = sources.get(insight.source) else {
                continue;
            };
            let text = source.text();
            let pragmas = pragma::collect(text);
            let start = insight.range.start as usize;
            if let Some(active) = pragma::suppressing(text, start, &self.rule.id, &pragmas) {
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
                trace:         insight.trace
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
    use std::path::PathBuf;

    use rustmanifest_schema::Severity;

    use super::*;

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
        assert!(matches!(error, EngineError::UnknownAstCheck { .. }));
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
        let findings = analyzer.analyze(&sources);
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
        assert!(analyzer.analyze(&sources).is_empty());
    }
}
