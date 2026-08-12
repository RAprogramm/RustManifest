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

use std::{
    fs,
    path::{Path, PathBuf}
};

use rustmanifest_engine::{SemanticAnalyzer, Source, Workspace};
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

/// Loads the fixture workspace of one side: the multi-file
/// `<side>_workspace/` directory when the rule ships one, otherwise the
/// single flat `<side>.rs` file.
fn workspace(dir: &Path, side: &str) -> Vec<Source> {
    let multi = dir.join(format!("{side}_workspace"));
    if multi.is_dir() {
        let mut sources = Vec::new();
        collect_rs(&multi, &mut sources);
        sources.sort_by_key(|source| source.path().to_path_buf());
        return sources;
    }
    let file = dir.join(format!("{side}.rs"));
    let text = fs::read_to_string(&file)
        .unwrap_or_else(|err| panic!("cannot read {}: {err}", file.display()));
    vec![Source::new(file, text)]
}

/// Recursively collects `.rs` files under `dir`.
fn collect_rs(dir: &Path, out: &mut Vec<Source>) {
    for entry in fs::read_dir(dir).expect("fixture workspace readable") {
        let path = entry.expect("fixture entry").path();
        if path.is_dir() {
            collect_rs(&path, out);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            let text = fs::read_to_string(&path).expect("fixture file readable");
            out.push(Source::new(path, text));
        }
    }
}

/// The module tree a resolver would state for a fixture workspace: a
/// crate root per `lib.rs` or `main.rs`, one module per further file named
/// after its stem, and one per inline module.
fn mounted(sources: &[Source]) -> rustmanifest_schema::Resolution {
    let anchored = sources.iter().any(|source| {
        source
            .path()
            .file_stem()
            .and_then(|held| held.to_str())
            .is_some_and(|stem| matches!(stem, "lib" | "main"))
    });
    let mut modules = Vec::new();
    for source in sources {
        let path = source.path();
        let Some(stem) = path.file_stem().and_then(|held| held.to_str()) else {
            continue;
        };
        let root = !anchored || matches!(stem, "lib" | "main");
        let names = if root {
            "fixture".to_owned()
        } else {
            format!("fixture::{stem}")
        };
        modules.push(rustmanifest_schema::ResolvedModule {
            names:      names.clone(),
            file:       path.to_path_buf(),
            crate_root: root,
            parent:     (!root).then(|| "fixture".to_owned())
        });
        let Ok(parsed) = syn::parse_file(source.text()) else {
            continue;
        };
        for item in &parsed.items {
            if let syn::Item::Mod(held) = item
                && held.content.is_some()
            {
                modules.push(rustmanifest_schema::ResolvedModule {
                    names:      format!("{names}::{}", held.ident),
                    file:       path.to_path_buf(),
                    crate_root: false,
                    parent:     Some(names.clone())
                });
            }
        }
    }
    let mut settled = rustmanifest_schema::Resolution {
        resolver: "fixture".to_owned(),
        modules,
        ..rustmanifest_schema::Resolution::default()
    };
    settled.settle();
    settled
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

        let fail_sources = workspace(&dir, "fail");
        let fail_tree = mounted(&fail_sources);
        let fail_findings = analyzer.analyze(&Workspace::resolved(&fail_sources, &fail_tree));
        assert!(
            !fail_findings.is_empty(),
            "{}: fail.rs MUST trigger the check but did not",
            rule.id
        );
        let pass_sources = workspace(&dir, "pass");
        let pass_tree = mounted(&pass_sources);
        let pass_findings = analyzer.analyze(&Workspace::resolved(&pass_sources, &pass_tree));
        assert!(
            pass_findings.is_empty(),
            "{}: pass.rs MUST NOT trigger the check but produced {:?}",
            rule.id,
            pass_findings
        );
    }
    assert!(seen >= 1, "pack must bundle at least one semantic rule");
}
