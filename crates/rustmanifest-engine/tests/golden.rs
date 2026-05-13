// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! End-to-end engine test on the in-tree rules pack fixtures.
//!
//! Each rule directory ships `pass.rs` and `fail.rs`; the orchestrator MUST
//! report at least one finding on `fail.rs` (for the rule under test) and
//! zero findings on `pass.rs`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]

use std::path::{Path, PathBuf};

use rustmanifest_engine::{OrchestratorBuilder, PatternAnalyzer};
use rustmanifest_rules_core::RULES;
use rustmanifest_schema::RuleDefinition;

fn rules_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("rustmanifest-rules-core")
        .join("rules")
}

fn orchestrator_for(rule: &rustmanifest_schema::Rule) -> rustmanifest_engine::Orchestrator {
    let analyzer = PatternAnalyzer::new(rule.clone()).unwrap();
    OrchestratorBuilder::new()
        .analyzer(Box::new(analyzer))
        .build()
}

#[test]
fn fail_fixtures_trigger_their_rule() {
    let rules_dir = rules_dir();
    for rule in RULES.iter() {
        if !matches!(rule.definition, RuleDefinition::Pattern { .. }) {
            continue;
        }
        let fail_path = rules_dir.join(&rule.id).join("fail.rs");
        let orchestrator = orchestrator_for(rule);
        let findings = orchestrator.run(std::slice::from_ref(&fail_path)).unwrap();
        let count = findings
            .iter()
            .filter(|finding| finding.rule_id == rule.id)
            .count();
        assert!(
            count >= 1,
            "rule {}: expected at least one finding on {}, got {count}",
            rule.id,
            fail_path.display()
        );
    }
}

#[test]
fn pass_fixtures_do_not_trigger_their_rule() {
    let rules_dir = rules_dir();
    for rule in RULES.iter() {
        if !matches!(rule.definition, RuleDefinition::Pattern { .. }) {
            continue;
        }
        let pass_path = rules_dir.join(&rule.id).join("pass.rs");
        let orchestrator = orchestrator_for(rule);
        let findings = orchestrator.run(std::slice::from_ref(&pass_path)).unwrap();
        let count = findings
            .iter()
            .filter(|finding| finding.rule_id == rule.id)
            .count();
        assert_eq!(
            count,
            0,
            "rule {}: expected zero findings on {}, got {}",
            rule.id,
            pass_path.display(),
            count
        );
    }
}

#[test]
fn full_rules_pack_runs_against_fixture_tree() {
    let rules_dir = rules_dir();
    let analyzers: Vec<Box<dyn rustmanifest_engine::Analyzer>> = RULES
        .iter()
        .filter(|rule| matches!(rule.definition, RuleDefinition::Pattern { .. }))
        .map(|rule| {
            let analyzer = PatternAnalyzer::new(rule.clone()).unwrap();
            Box::new(analyzer) as Box<dyn rustmanifest_engine::Analyzer>
        })
        .collect();
    let orchestrator = OrchestratorBuilder::new().analyzers(analyzers).build();

    let mut paths = Vec::new();
    collect_rs(&rules_dir, &mut paths);
    let findings = orchestrator.run(&paths).unwrap();
    assert!(
        !findings.is_empty(),
        "expected the bundled fail.rs fixtures to produce findings"
    );
}

fn collect_rs(dir: &Path, sink: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs(&path, sink);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            sink.push(path);
        }
    }
}
