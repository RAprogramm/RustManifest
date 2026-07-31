// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Fixture conformance for every bundled `semantic`-tier rule: `fail.rs`
//! MUST produce at least one finding and `pass.rs` MUST produce none, each
//! evaluated as a one-file workspace.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]

use std::{fs, path::PathBuf};

use rustmanifest_engine::{SemanticAnalyzer, Source};
use rustmanifest_rules_core::RULES;
use rustmanifest_schema::RuleDefinition;

fn rules_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("engine crate has a parent directory")
        .join("rustmanifest-rules-core")
        .join("rules")
}

#[test]
fn every_semantic_rule_resolves_to_a_built_in_check() {
    for rule in RULES.iter() {
        if matches!(rule.definition, RuleDefinition::Semantic { .. }) {
            let built = SemanticAnalyzer::new(rule.clone());
            assert!(
                built.is_ok(),
                "rule {} names an unknown semantic check: {:?}",
                rule.id,
                built.err()
            );
        }
    }
}

#[test]
fn semantic_fixtures_match_their_rule() {
    let rules_dir = rules_dir();
    let mut seen = 0;
    for rule in RULES.iter() {
        if !matches!(rule.definition, RuleDefinition::Semantic { .. }) {
            continue;
        }
        seen += 1;
        let analyzer = SemanticAnalyzer::new(rule.clone()).unwrap();
        let dir = rules_dir.join(&rule.id);

        let fail_src = fs::read_to_string(dir.join("fail.rs"))
            .unwrap_or_else(|err| panic!("{}: cannot read fail.rs: {err}", rule.id));
        let pass_src = fs::read_to_string(dir.join("pass.rs"))
            .unwrap_or_else(|err| panic!("{}: cannot read pass.rs: {err}", rule.id));

        let fail_findings = analyzer.analyze(&[Source::new(dir.join("fail.rs"), fail_src)]);
        assert!(
            !fail_findings.is_empty(),
            "{}: fail.rs MUST trigger the check but did not",
            rule.id
        );
        let pass_findings = analyzer.analyze(&[Source::new(dir.join("pass.rs"), pass_src)]);
        assert!(
            pass_findings.is_empty(),
            "{}: pass.rs MUST NOT trigger the check but produced {:?}",
            rule.id,
            pass_findings
        );
    }
    assert!(seen >= 1, "pack must bundle at least one semantic rule");
}
