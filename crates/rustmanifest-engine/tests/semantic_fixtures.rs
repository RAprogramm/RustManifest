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
use rustmanifest_schema::{ResolvedMode, RuleDefinition};

/// The receiver modes a resolver would state for the calls of one rule's
/// fixtures, by rule and by the call as it is written.
///
/// A fixture is a loose file and no resolver runs over it here, so a rule
/// that reads how a call takes the value it stands on refuses on every
/// fixture. Writing the declarations down keeps those rules judged, and
/// keeps the fabrication in the harness where it is visible rather than in
/// the engine, which is where a table of method names used to answer this.
const SETTLED_RECEIVERS: [(&str, &[(&str, ResolvedMode)]); 1] = [(
    "RM-MEM-003",
    &[
        ("line.push('#')", ResolvedMode::BorrowedMut),
        ("line.len()", ResolvedMode::Borrowed)
    ]
)];

/// The fields a resolver would name for one rule's fixtures, by rule and
/// by the access as it is written: the type holding it and the type it
/// carries.
///
/// The same fabrication as the receiver modes above, for the same reason.
/// A field access says `self.hits`, and which type that `self` is, is the
/// compiler's answer; a rule reading what a body writes has nothing to
/// join on until it is stated.
const SETTLED_FIELDS: [(&str, &[SettledField]); 1] =
    [("RM-QUAL-002", &[("self.hits", "Counter", "u64")])];

/// One field a resolver names: the access as it is written, the type
/// holding it, and the type it carries.
type SettledField = (&'static str, &'static str, &'static str);

/// Adds the fields written for `rule` to a fixture's resolution, at every
/// place the fixtures write them.
fn settle_fields(rule: &str, sources: &[Source], into: &mut rustmanifest_schema::Resolution) {
    let Some((_, fields)) = SETTLED_FIELDS.iter().find(|(named, _)| *named == rule) else {
        return;
    };
    for source in sources {
        for (written, owner, produced) in *fields {
            let mut cursor = 0usize;
            while let Some(found) = source.text()[cursor..].find(written) {
                let at = cursor + found;
                cursor = at + written.len();
                let start = u32::try_from(at).expect("a fixture is small");
                into.fields.push(rustmanifest_schema::ResolvedField {
                    at:       rustmanifest_schema::Location {
                        file:  source.path().to_path_buf(),
                        range: rustmanifest_schema::Range {
                            start,
                            end: start
                                + u32::try_from(written.len()).expect("a fixture is small")
                        }
                    },
                    owner:    (*owner).to_owned(),
                    produced: (*produced).to_owned()
                });
            }
        }
    }
    into.settle();
}

/// Adds the receiver modes written for `rule` to a fixture's resolution.
fn settle_receivers(rule: &str, sources: &[Source], into: &mut rustmanifest_schema::Resolution) {
    let Some((_, calls)) = SETTLED_RECEIVERS.iter().find(|(named, _)| *named == rule) else {
        return;
    };
    for source in sources {
        for (written, mode) in *calls {
            let Some(found) = source.text().find(written) else {
                continue;
            };
            let start = u32::try_from(found).expect("a fixture is small");
            into.calls.push(rustmanifest_schema::ResolvedCall {
                at:       rustmanifest_schema::Location {
                    file:  source.path().to_path_buf(),
                    range: rustmanifest_schema::Range {
                        start,
                        end: start + u32::try_from(written.len()).expect("a fixture is small")
                    }
                },
                callee:   (*written).to_owned(),
                returns:  "()".to_owned(),
                receiver: None,
                receiver_path: None,
                reaches:  None,
                takes:    Some(*mode)
            });
        }
    }
    into.settle();
}

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
    let paths = reached(sources);
    let mut settled = rustmanifest_schema::Resolution {
        resolver: "fixture".to_owned(),
        modules,
        paths,
        ..rustmanifest_schema::Resolution::default()
    };
    settled.settle();
    settled
}

/// What each `crate::`-rooted import of a fixture reaches, stated the way
/// a resolver would.
fn reached(sources: &[Source]) -> Vec<rustmanifest_schema::ResolvedPath> {
    let mut rows = Vec::new();
    for source in sources {
        let text = source.text();
        let mut cursor = 0usize;
        while let Some(found) = text[cursor..].find("crate::") {
            let start = cursor + found + "crate::".len();
            let end = start
                + text[start..]
                    .find(|held: char| !held.is_alphanumeric() && held != '_' && held != ':')
                    .unwrap_or(text.len() - start);
            let written = &text[start..end];
            cursor = end;
            let Some(leaf) = written.rsplit("::").next() else {
                continue;
            };
            let at = end - leaf.len();
            rows.push(rustmanifest_schema::ResolvedPath {
                at:        rustmanifest_schema::Location {
                    file:  source.path().to_path_buf(),
                    range: rustmanifest_schema::Range {
                        start: u32::try_from(at).unwrap_or(u32::MAX),
                        end:   u32::try_from(end).unwrap_or(u32::MAX)
                    }
                },
                names:     format!("fixture::{written}"),
                kind:      "struct".to_owned(),
                declaring: "fixture".to_owned()
            });
        }
    }
    rows
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
        let mut fail_tree = mounted(&fail_sources);
        settle_receivers(&rule.id, &fail_sources, &mut fail_tree);
        settle_fields(&rule.id, &fail_sources, &mut fail_tree);
        let fail_findings = analyzer.analyze(&Workspace::resolved(&fail_sources, &fail_tree));
        assert!(
            !fail_findings.is_empty(),
            "{}: fail.rs MUST trigger the check but did not",
            rule.id
        );
        let pass_sources = workspace(&dir, "pass");
        let mut pass_tree = mounted(&pass_sources);
        settle_receivers(&rule.id, &pass_sources, &mut pass_tree);
        settle_fields(&rule.id, &pass_sources, &mut pass_tree);
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
