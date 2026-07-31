// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Integration tests for the bundled rules pack.
//!
//! These tests load the embedded `RULES` and the source-tree fixtures and
//! assert that every rule's regex behaves as advertised on its `pass.rs` and
//! `fail.rs` files. They also verify pack-level invariants (unique IDs, ID
//! shape) that the build script enforces at build time — duplicating the
//! check here protects against future regressions to the build script
//! itself.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]

use std::{collections::HashSet, fs, path::PathBuf};

use regex::Regex;
use rustmanifest_rules_core::RULES;
use rustmanifest_schema::RuleDefinition;

fn rules_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("rules")
}

#[test]
fn pack_is_not_empty() {
    assert!(!RULES.is_empty(), "rules pack must not be empty");
}

#[test]
fn ids_are_unique() {
    let mut seen: HashSet<&str> = HashSet::with_capacity(RULES.len());
    for rule in RULES.iter() {
        assert!(
            seen.insert(rule.id.as_str()),
            "duplicate rule id detected in pack: {}",
            rule.id
        );
    }
}

#[test]
fn ids_follow_naming_convention() {
    let id_re = Regex::new(r"^RM-(SEC|PERF|RUST|QUAL|STRUCT)-\d{3}$").unwrap();
    for rule in RULES.iter() {
        assert!(
            id_re.is_match(&rule.id),
            "rule id {:?} violates RM-(SEC|PERF|RUST|QUAL|STRUCT)-NNN",
            rule.id
        );
    }
}

#[test]
fn pattern_rules_compile() {
    for rule in RULES.iter() {
        if let RuleDefinition::Pattern {
            regex, ..
        } = &rule.definition
        {
            let compiled = Regex::new(regex);
            assert!(
                compiled.is_ok(),
                "rule {} regex failed to compile: {:?}",
                rule.id,
                compiled.err()
            );
        }
    }
}

#[test]
fn pattern_fixtures_match_their_rule() {
    let rules_dir = rules_dir();
    for rule in RULES.iter() {
        let RuleDefinition::Pattern {
            regex, ..
        } = &rule.definition
        else {
            continue;
        };
        let re = Regex::new(regex).unwrap();
        let dir = rules_dir.join(&rule.id);

        let fail_src = fs::read_to_string(dir.join("fail.rs"))
            .unwrap_or_else(|err| panic!("{}: cannot read fail.rs: {err}", rule.id));
        let pass_src = fs::read_to_string(dir.join("pass.rs"))
            .unwrap_or_else(|err| panic!("{}: cannot read pass.rs: {err}", rule.id));

        assert!(
            re.is_match(&fail_src),
            "{}: fail.rs MUST trigger the regex but did not",
            rule.id
        );
        assert!(
            !re.is_match(&pass_src),
            "{}: pass.rs MUST NOT trigger the regex but did",
            rule.id
        );
    }
}

#[test]
fn rule_id_matches_directory_name() {
    let rules_dir = rules_dir();
    for rule in RULES.iter() {
        let dir = rules_dir.join(&rule.id);
        assert!(
            dir.is_dir(),
            "rule {}: expected directory {}",
            rule.id,
            dir.display()
        );
    }
}
