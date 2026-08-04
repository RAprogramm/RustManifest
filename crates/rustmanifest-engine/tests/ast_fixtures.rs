// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Fixture conformance for every bundled `ast`-tier rule: `fail.rs` MUST
//! produce at least one finding and `pass.rs` MUST produce none. This mirrors
//! the regex fixture test in `rustmanifest-rules-core`, which cannot depend
//! on the engine without a dependency cycle.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]

use std::{fs, path::PathBuf};

use rustmanifest_engine::{Analyzer, AstAnalyzer, Source};
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
fn pack_contains_ast_rules() {
    let count = RULES
        .iter()
        .filter(|rule| matches!(rule.definition, RuleDefinition::Ast { .. }))
        .count();
    assert!(count >= 8, "expected at least 8 ast rules, found {count}");
}

#[test]
fn every_ast_rule_resolves_to_a_built_in_check() {
    for rule in RULES.iter() {
        if matches!(rule.definition, RuleDefinition::Ast { .. }) {
            let built = AstAnalyzer::new(rule.clone());
            assert!(
                built.is_ok(),
                "rule {} names an unknown ast check: {:?}",
                rule.id,
                built.err()
            );
        }
    }
}

#[test]
fn ast_fixtures_match_their_rule() {
    let rules_dir = rules_dir();
    for rule in RULES.iter() {
        if !matches!(rule.definition, RuleDefinition::Ast { .. }) {
            continue;
        }
        let analyzer = AstAnalyzer::new(rule.clone()).unwrap();
        let dir = rules_dir.join(&rule.id);

        let fail_src = fs::read_to_string(dir.join("fail.rs"))
            .unwrap_or_else(|err| panic!("{}: cannot read fail.rs: {err}", rule.id));
        let pass_src = fs::read_to_string(dir.join("pass.rs"))
            .unwrap_or_else(|err| panic!("{}: cannot read pass.rs: {err}", rule.id));

        let fail_findings = analyzer.analyze(&Source::new(dir.join("fail.rs"), fail_src));
        assert!(
            !fail_findings.is_empty(),
            "{}: fail.rs MUST trigger the check but did not",
            rule.id
        );
        let pass_findings = analyzer.analyze(&Source::new(dir.join("pass.rs"), pass_src));
        assert!(
            pass_findings.is_empty(),
            "{}: pass.rs MUST NOT trigger the check but produced {:?}",
            rule.id,
            pass_findings
        );
    }
}
