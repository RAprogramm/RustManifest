// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Which rules of the bundled pack the configuration leaves enabled, and
//! what they find in the sources.

use rustmanifest_config::Effective;
use rustmanifest_engine::Source;
use rustmanifest_rules_core::RULES;
use rustmanifest_schema::{Finding, RuleDefinition};

/// Runs every enabled rule of the bundled pack over the sources.
pub(super) fn analyze(sources: &[Source]) -> Vec<Finding> {
    let Ok(config) = rustmanifest_config::load(None) else {
        return Vec::new();
    };
    let Ok(effective) = Effective::from_config(config) else {
        return Vec::new();
    };
    let analyzers: Vec<rustmanifest_engine::SemanticAnalyzer> = RULES
        .iter()
        .filter(|rule| effective.rule_enabled(rule))
        .filter(|rule| matches!(rule.definition, RuleDefinition::Semantic { .. }))
        .filter_map(|rule| rustmanifest_engine::SemanticAnalyzer::new(rule.clone()).ok())
        .collect();
    rustmanifest_engine::semantic_findings(&analyzers, sources)
}
