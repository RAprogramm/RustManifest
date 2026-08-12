// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The compiler's verdict on every synthesized edit: a fix, an
//! extraction plan, or an installed generation, applied, must yield a
//! program `rustc` accepts.
//!
//! The gates a synthesis checks before producing an edit are claims
//! about the engine's own preconditions; this harness is the other
//! half of the definition of correctness, the same half the corpus
//! rerun is for a rule's silence: the applied result handed to the
//! real compiler, whose rejection fails the test with its own words.
//! Every bundled semantic rule's `fail.rs` fixture is the bed: each
//! finding's fixes are applied alone (an applier owes the edits of one
//! finding together, and nothing across findings), and multi-file or
//! renaming fixes are skipped here with their skip counted, so a
//! silent shrink of coverage cannot read as green. The generated impl
//! blocks land only as editor edits the author applies — the engine
//! writes no source byte to disk — so the text the montage confirms in
//! memory is judged here exactly as it would land: appended to the
//! defining file and handed whole to the compiler.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]

use std::{fs, path::PathBuf, process::Command};

use rustmanifest_engine::{SemanticAnalyzer, Source, Workspace};
use rustmanifest_rules_core::RULES;
use rustmanifest_schema::{Finding, RuleDefinition};

/// Compiles one program (a root file plus any module files beside it)
/// and returns the compiler's complaint, if any.
fn rejected(root_name: &str, files: &[(PathBuf, String)]) -> Option<String> {
    let bed = tempfile::tempdir().expect("a temporary directory");
    for (path, text) in files {
        let landed = bed.path().join(path);
        if let Some(parent) = landed.parent() {
            fs::create_dir_all(parent).expect("module directories");
        }
        fs::write(landed, text).expect("writing a fixture file");
    }
    let built = Command::new("rustc")
        .arg("--edition")
        .arg("2024")
        .arg("--crate-type")
        .arg("lib")
        .arg("--emit")
        .arg("metadata")
        .arg("-A")
        .arg("warnings")
        .arg("-o")
        .arg(bed.path().join("out.rmeta"))
        .arg(bed.path().join(root_name))
        .output()
        .expect("running rustc");
    if built.status.success() {
        None
    } else {
        Some(String::from_utf8_lossy(&built.stderr).into_owned())
    }
}

/// One finding's fixes applied to the fixture text, byte edits only.
fn applied(text: &str, finding: &Finding) -> String {
    let mut fixes = finding.fixes.clone();
    fixes.sort_by_key(|fix| std::cmp::Reverse(fix.range.start));
    let mut held = text.to_owned();
    for fix in fixes {
        held.replace_range(
            fix.range.start as usize..fix.range.end as usize,
            &fix.replacement
        );
    }
    held
}

/// Every single-file fix a bundled rule synthesizes on its own `fail`
/// fixture must compile once applied.
#[test]
fn every_applied_fix_survives_the_compiler() {
    let rules_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("engine crate has a parent")
        .join("rustmanifest-rules-core")
        .join("rules");
    let mut compiled = 0u32;
    let mut skipped = 0u32;
    for rule in RULES.iter() {
        if !matches!(rule.definition, RuleDefinition::Semantic { .. }) {
            continue;
        }
        let fixture = rules_dir.join(&rule.id).join("fail.rs");
        let Ok(text) = fs::read_to_string(&fixture) else {
            continue;
        };
        let sources = vec![Source::new(PathBuf::from("src/lib.rs"), text.clone())];
        let workspace = Workspace::new(&sources);
        let analyzer = SemanticAnalyzer::new(rule.clone()).expect("a bundled rule builds");
        for finding in analyzer.analyze(&workspace) {
            if finding.fixes.is_empty() {
                continue;
            }
            let portable = finding
                .fixes
                .iter()
                .all(|fix| fix.file.is_none() && fix.rename_to.is_none());
            if !portable {
                skipped += 1;
                continue;
            }
            let landed = applied(&text, &finding);
            if let Some(complaint) = rejected(
                "src/lib.rs",
                &[(PathBuf::from("src/lib.rs"), landed.clone())]
            ) {
                panic!(
                    "rule {} produced a fix whose applied result the compiler rejects:\n\
                     --- applied ---\n{landed}\n--- rustc ---\n{complaint}",
                    rule.id
                );
            }
            compiled += 1;
        }
    }
    assert!(
        compiled >= 2,
        "the harness must have compiled at least the ownership and buffering fixes; \
         compiled {compiled}, skipped {skipped}"
    );
}

/// An installed canon impl — the same appended text the editor edit
/// lands — must compile, for both canon traits over proven fields.
#[test]
fn an_installed_canon_impl_survives_the_compiler() {
    let text = "pub struct User {\n    pub name: String,\n    pub age: u8,\n}\n";
    let sources = vec![Source::new(PathBuf::from("src/lib.rs"), text.to_owned())];
    for trait_name in ["Default", "Display"] {
        let gain = rustmanifest_engine::semantic::install_impl(&sources, 0, "User", trait_name)
            .expect("a canon install over proven fields");
        if let Some(complaint) = rejected(
            "src/lib.rs",
            &[(PathBuf::from("src/lib.rs"), gain.installed.clone())]
        ) {
            panic!(
                "the installed `{trait_name}` impl does not compile:\n--- installed ---\n{}\n\
                 --- rustc ---\n{complaint}",
                gain.installed
            );
        }
    }
}

/// A skeleton obligation pays its debt in panic, not in rejection: the
/// installed text still compiles, and the gain names the open debt.
#[test]
fn an_installed_skeleton_compiles_with_its_panic_debt_open() {
    let text = "pub struct Batch {\n    pub held: Vec<String>,\n}\n";
    let sources = vec![Source::new(PathBuf::from("src/lib.rs"), text.to_owned())];
    let gain = rustmanifest_engine::semantic::install_impl(&sources, 0, "Batch", "Display")
        .expect("a skeleton install");
    assert!(gain.panics, "{}", gain.installed);
    if let Some(complaint) = rejected(
        "src/lib.rs",
        &[(PathBuf::from("src/lib.rs"), gain.installed.clone())]
    ) {
        panic!(
            "the installed skeleton does not compile:\n--- installed ---\n{}\n--- rustc \
             ---\n{complaint}",
            gain.installed
        );
    }
}

/// Every fulfilled intent's confirmed installation must compile: the
/// chain from a written call to an installed block ends at the
/// compiler, not at the re-reading.
#[test]
fn every_fulfilled_intent_survives_the_compiler() {
    let text = "pub struct Job {\n    pub name: String,\n}\n\npub fn make() -> Job {\n    \
                Job::default()\n}\n";
    let sources = vec![Source::new(PathBuf::from("src/lib.rs"), text.to_owned())];
    let fulfilled = rustmanifest_engine::semantic::fulfill_intents(&sources);
    assert!(!fulfilled.is_empty(), "the written call states an intent");
    for held in fulfilled {
        let gain = held.gain.expect("a confirmed gain");
        if let Some(complaint) = rejected(
            "src/lib.rs",
            &[(PathBuf::from("src/lib.rs"), gain.installed.clone())]
        ) {
            panic!(
                "a fulfilled intent's installation does not compile:\n--- installed ---\n{}\n\
                 --- rustc ---\n{complaint}",
                gain.installed
            );
        }
    }
}
