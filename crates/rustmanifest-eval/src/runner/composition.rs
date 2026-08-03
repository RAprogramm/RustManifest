// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Analyzer composition: turns the bundled rule catalog into the two
//! shapes the corpus run needs — one orchestrator for the per-file rules
//! and one analyzer per repository-wide semantic rule.

use rustmanifest_engine::{
    Analyzer, AstAnalyzer, Orchestrator, OrchestratorBuilder, PatternAnalyzer, SemanticAnalyzer
};
use rustmanifest_rules_core::RULES;
use rustmanifest_schema::RuleDefinition;

use crate::error::EvalError;

/// Builds one orchestrator holding every bundled pattern and AST rule.
pub(super) fn build_orchestrator() -> Result<Orchestrator, EvalError> {
    let analyzers: Vec<Box<dyn Analyzer>> = RULES
        .iter()
        .filter_map(|rule| match rule.definition {
            RuleDefinition::Pattern {
                ..
            } => Some(
                PatternAnalyzer::new(rule.clone())
                    .map(|analyzer| Box::new(analyzer) as Box<dyn Analyzer>)
            ),
            RuleDefinition::Ast {
                ..
            } => Some(
                AstAnalyzer::new(rule.clone())
                    .map(|analyzer| Box::new(analyzer) as Box<dyn Analyzer>)
            ),
            RuleDefinition::Semantic {
                ..
            } => None
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(OrchestratorBuilder::new().analyzers(analyzers).build())
}

/// Builds one analyzer per bundled semantic rule.
pub(super) fn build_semantic() -> Result<Vec<SemanticAnalyzer>, EvalError> {
    Ok(RULES
        .iter()
        .filter(|rule| matches!(rule.definition, RuleDefinition::Semantic { .. }))
        .map(|rule| SemanticAnalyzer::new(rule.clone()))
        .collect::<Result<Vec<_>, _>>()?)
}
