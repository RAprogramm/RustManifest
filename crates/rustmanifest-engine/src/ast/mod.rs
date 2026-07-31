// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Tier-2 AST analyzer: built-in `syn`-based structural checks.
//!
//! Each rule of the `ast` tier names one built-in check. The analyzer parses
//! a source file once, hands the tree to the check, and converts the
//! occurrences it reports into findings, honoring inline pragma suppression
//! exactly like the pattern tier.

mod api_size;
mod constant_encapsulation;
mod constructor_design;
mod delegating_constructors;
mod entity_naming;
mod fakes_over_mocks;
mod immutability_first;
mod method_naming;

use rustmanifest_schema::{Finding, Location, Range, Rule, RuleDefinition};
use tracing::trace;

use crate::{analyzer::Analyzer, error::EngineError, pragma, source::Source};

/// One structural violation reported by a built-in check.
pub(crate) struct Occurrence {
    /// Byte range of the offending item.
    range: Range,
    /// Check-specific description of the violation.
    detail: String,
}

impl Occurrence {
    /// Creates an occurrence for the item spanned by `span`.
    pub(crate) fn new(span: &proc_macro2::Span, detail: String) -> Self {
        let bytes = span.byte_range();
        Self {
            range: Range {
                start: u32::try_from(bytes.start).unwrap_or(u32::MAX),
                end: u32::try_from(bytes.end).unwrap_or(u32::MAX),
            },
            detail,
        }
    }
}

/// Built-in structural checks addressable from `definition.ast.check`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Check {
    /// `entity-naming` — agent-suffixed type names (`STRUCTURE.md` §1).
    EntityNaming,
    /// `method-naming` — redundant `get_` accessor prefix (§2).
    MethodNaming,
    /// `api-size` — inherent impls exposing too many public methods (§4).
    ApiSize,
    /// `constructor-design` — constructors doing more than assignment (§5).
    ConstructorDesign,
    /// `delegating-constructors` — constructors duplicating `Self { .. }`
    /// instead of delegating to one primary constructor (§6).
    DelegatingConstructors,
    /// `immutability-first` — `set_*` mutators taking `&mut self` (§7).
    ImmutabilityFirst,
    /// `constant-encapsulation` — module-level `pub const` items (§8).
    ConstantEncapsulation,
    /// `fakes-over-mocks` — mock-library imports and `#[automock]` (§9).
    FakesOverMocks,
}

impl Check {
    /// Resolves a check identifier from the rules pack.
    fn resolved(name: &str) -> Option<Self> {
        match name {
            "entity-naming" => Some(Self::EntityNaming),
            "method-naming" => Some(Self::MethodNaming),
            "api-size" => Some(Self::ApiSize),
            "constructor-design" => Some(Self::ConstructorDesign),
            "delegating-constructors" => Some(Self::DelegatingConstructors),
            "immutability-first" => Some(Self::ImmutabilityFirst),
            "constant-encapsulation" => Some(Self::ConstantEncapsulation),
            "fakes-over-mocks" => Some(Self::FakesOverMocks),
            _ => None,
        }
    }

    /// Runs the check over a parsed file.
    fn occurrences(self, file: &syn::File) -> Vec<Occurrence> {
        match self {
            Self::EntityNaming => entity_naming::occurrences(file),
            Self::MethodNaming => method_naming::occurrences(file),
            Self::ApiSize => api_size::occurrences(file),
            Self::ConstructorDesign => constructor_design::occurrences(file),
            Self::DelegatingConstructors => delegating_constructors::occurrences(file),
            Self::ImmutabilityFirst => immutability_first::occurrences(file),
            Self::ConstantEncapsulation => constant_encapsulation::occurrences(file),
            Self::FakesOverMocks => fakes_over_mocks::occurrences(file),
        }
    }
}

/// Tier-2 analyzer that applies one built-in structural check per rule.
#[derive(Debug)]
pub struct AstAnalyzer {
    rule: Rule,
    check: Check,
}

impl AstAnalyzer {
    /// Builds an analyzer for the given `ast`-tier rule.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::UnknownAstCheck`] if the rule is not an
    /// `ast`-tier rule or names a check this engine build does not provide.
    pub fn new(rule: Rule) -> Result<Self, EngineError> {
        let RuleDefinition::Ast { ref check } = rule.definition else {
            return Err(EngineError::UnknownAstCheck {
                rule_id: rule.id,
                check: "<not an ast-tier rule>".to_owned(),
            });
        };
        let Some(check) = Check::resolved(check) else {
            return Err(EngineError::UnknownAstCheck {
                rule_id: rule.id,
                check: check.clone(),
            });
        };
        Ok(Self { rule, check })
    }
}

impl Analyzer for AstAnalyzer {
    fn rule(&self) -> &Rule {
        &self.rule
    }

    fn analyze(&self, source: &Source) -> Vec<Finding> {
        let text = source.text();
        let Ok(file) = syn::parse_file(text) else {
            trace!(
                rule = %self.rule.id,
                path = %source.path().display(),
                "file does not parse as Rust; skipping ast check"
            );
            return Vec::new();
        };

        let pragmas = pragma::collect(text);
        let mut findings = Vec::new();
        for occurrence in self.check.occurrences(&file) {
            let start = occurrence.range.start as usize;
            if let Some(active) = pragma::suppressing(text, start, &self.rule.id, &pragmas) {
                source.record_pragma_use(active.line_index);
                trace!(rule = %self.rule.id, offset = start, "suppressed by pragma");
                continue;
            }
            findings.push(Finding {
                rule_id: self.rule.id.clone(),
                severity: self.rule.severity,
                location: Location {
                    file: source.path().to_path_buf(),
                    range: occurrence.range,
                },
                message: format!("{}: {}", self.rule.title, occurrence.detail),
                rationale_uri: self.rule.rationale_uri.clone(),
            });
        }
        findings
    }
}

/// Appends `items` and the items of every nested inline module to `out`.
pub(crate) fn flattened<'a>(items: &'a [syn::Item], out: &mut Vec<&'a syn::Item>) {
    for item in items {
        out.push(item);
        if let syn::Item::Mod(module) = item
            && let Some((_, nested)) = &module.content
        {
            flattened(nested, out);
        }
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
            id: "RM-STRUCT-001".to_owned(),
            severity: Severity::Warning,
            title: "Entity naming".to_owned(),
            rationale_uri: "rustmanifest://methodology/structure#entity-naming".to_owned(),
            definition: RuleDefinition::Ast {
                check: check.to_owned(),
            },
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
            regex: "x".to_owned(),
            exclude_globs: Vec::new(),
        };
        assert!(AstAnalyzer::new(rule).is_err());
    }

    #[test]
    fn unparsable_source_yields_no_findings() {
        let analyzer = AstAnalyzer::new(ast_rule("entity-naming")).unwrap();
        let source = Source::new(PathBuf::from("src/lib.rs"), "fn broken(".to_owned());
        assert!(analyzer.analyze(&source).is_empty());
    }

    #[test]
    fn finding_carries_rule_metadata_and_range() {
        let analyzer = AstAnalyzer::new(ast_rule("entity-naming")).unwrap();
        let source = Source::new(
            PathBuf::from("src/lib.rs"),
            "pub struct ConfigLoader;\n".to_owned(),
        );
        let findings = analyzer.analyze(&source);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_id, "RM-STRUCT-001");
        assert!(findings[0].location.range.end > findings[0].location.range.start);
    }

    #[test]
    fn pragma_suppresses_ast_finding() {
        let analyzer = AstAnalyzer::new(ast_rule("entity-naming")).unwrap();
        let text = "// rustmanifest: allow(RM-STRUCT-001) reason=\"fixture\"\npub struct \
                    ConfigLoader;\n";
        let source = Source::new(PathBuf::from("src/lib.rs"), text.to_owned());
        assert!(analyzer.analyze(&source).is_empty());
    }

    #[test]
    fn nested_module_items_are_flattened() {
        let file = syn::parse_file("mod outer { mod inner { struct DataManager; } }").unwrap();
        let mut items = Vec::new();
        flattened(&file.items, &mut items);
        assert_eq!(items.len(), 3);
    }
}
