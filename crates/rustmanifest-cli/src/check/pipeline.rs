// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! One pass of the analysis pipeline: configuration, analyzer construction,
//! discovery, the three tiers, delegation, and the severity filter.
//!
//! The workspace-level tiers — semantic checks and declared invariants —
//! read one set of sources between them, loaded only when at least one of
//! them has something to say. Loading it per tier read every file from disk
//! twice, and loading it unconditionally read them all for a run that used
//! neither tier.

use anyhow::Context;
use rustmanifest_config::Effective;
use rustmanifest_engine::{
    AstAnalyzer, FindingsCache, OrchestratorBuilder, PatternAnalyzer, SemanticAnalyzer, walker
};
use rustmanifest_rules_core::RULES;
use rustmanifest_schema::{Finding, Rule, RuleDefinition};
use tracing::info;

use super::{
    delegation::delegated_clippy_findings, globs::apply_globs, options::CheckArgs,
    severity::apply_severity_overrides
};
use crate::read_sources;

/// Runs the full analysis pipeline once and returns the filtered findings
/// with the effective configuration.
pub(super) fn collect_findings(args: &CheckArgs) -> anyhow::Result<(Vec<Finding>, Effective)> {
    let config =
        rustmanifest_config::load(args.config.as_deref()).context("loading configuration")?;
    let effective = Effective::from_config(config).context("resolving configuration profile")?;

    let enabled: Vec<&Rule> = RULES
        .iter()
        .filter(|rule| effective.rule_enabled(rule))
        .collect();

    let analyzers: Vec<Box<dyn rustmanifest_engine::Analyzer>> = enabled
        .iter()
        .copied()
        .filter_map(|rule| match rule.definition {
            RuleDefinition::Pattern {
                ..
            } => Some(
                PatternAnalyzer::new(rule.clone())
                    .map(|analyzer| Box::new(analyzer) as Box<dyn rustmanifest_engine::Analyzer>)
            ),
            RuleDefinition::Ast {
                ..
            } => Some(
                AstAnalyzer::new(rule.clone())
                    .map(|analyzer| Box::new(analyzer) as Box<dyn rustmanifest_engine::Analyzer>)
            ),
            RuleDefinition::Semantic {
                ..
            } => None
        })
        .collect::<Result<Vec<_>, _>>()
        .context("building analyzers from bundled rules")?;

    let semantic: Vec<SemanticAnalyzer> = enabled
        .iter()
        .copied()
        .filter(|rule| matches!(rule.definition, RuleDefinition::Semantic { .. }))
        .map(|rule| SemanticAnalyzer::new(rule.clone()))
        .collect::<Result<Vec<_>, _>>()
        .context("building semantic analyzers from bundled rules")?;

    info!(
        analyzer_count = analyzers.len(),
        path_count = args.paths.len(),
        "running check"
    );

    let cache = args
        .cache
        .as_deref()
        .map(|dir| {
            FindingsCache::open(dir)
                .with_context(|| format!("opening findings cache at {}", dir.display()))
        })
        .transpose()?;

    let orchestrator = OrchestratorBuilder::new()
        .analyzers(analyzers)
        .max_file_bytes(args.max_file_size)
        .cache(cache)
        .build();

    let discovered = walker::discover(&args.paths).context("file discovery failed")?;
    let files = apply_globs(discovered, &effective)?;
    let mut findings = orchestrator
        .run(&files)
        .context("orchestrator run failed")?;
    let sources = if semantic.is_empty() && effective.invariants().is_empty() {
        Vec::new()
    } else {
        read_sources(&files, args.max_file_size)
    };
    let workspace = rustmanifest_engine::Workspace::new(&sources);
    findings.extend(rustmanifest_engine::semantic_findings(
        &semantic, &workspace
    ));
    if !effective.invariants().is_empty() {
        let scip = args
            .scip
            .as_deref()
            .map(rustmanifest_engine::ScipEdges::from_file)
            .transpose()
            .context("loading SCIP index")?;
        findings.extend(rustmanifest_engine::check_invariants_with(
            &sources,
            effective.invariants(),
            scip.as_ref()
        ));
    }
    if args.delegate_clippy {
        findings.extend(delegated_clippy_findings(&args.paths));
    }
    apply_severity_overrides(&mut findings, &effective);
    findings.retain(|finding| args.severity_filter.admits(finding.severity));
    Ok((findings, effective))
}
