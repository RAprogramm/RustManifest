// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Integration tests for `check --fix`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]

use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn rustmanifest() -> Command {
    Command::cargo_bin("rustmanifest").unwrap()
}

fn tree_with_unused_pragma() -> TempDir {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(
        dir.path().join("src/lib.rs"),
        "// rustmanifest: allow(RM-SEC-001) reason=\"stale\"\npub fn plain() -> u64 { 7 }\n"
    )
    .unwrap();
    dir
}


/// Writes beside `tree` what a compiler settles about its sources, and
/// answers with the path the check reads it from.
///
/// Every semantic reading answers from the compiler and from nothing
/// else, so a run over a bare directory states no finding at all. The
/// engine's own fixture settler states the answers a compiler would
/// give about a file of a few lines, which is what these trees are.
fn settled_overlay(tree: &TempDir) -> std::path::PathBuf {
    let files = rustmanifest_engine::walker::discover(&[tree.path().to_path_buf()])
        .expect("the tree is walkable");
    let sources: Vec<rustmanifest_engine::Source> = files
        .iter()
        .filter_map(|path| {
            let text = fs::read_to_string(path).ok()?;
            let named = path.strip_prefix(tree.path()).unwrap_or(path);
            Some(rustmanifest_engine::Source::new(named.to_path_buf(), text))
        })
        .collect();
    let settled = settled_over(&sources);
    let at = tree.path().join("resolution.json");
    fs::write(&at, serde_json::to_string(&settled).expect("the overlay serializes"))
        .expect("the overlay is written");
    at
}

#[test]
fn unused_pragma_is_reported_with_a_machine_fix() {
    rustmanifest()
        .arg("check")
        .arg("--format")
        .arg("json")
        .arg(tree_with_unused_pragma().path())
        .assert()
        .failure()
        .stdout(predicate::str::contains("RM-META-001"))
        .stdout(predicate::str::contains("machine-applicable"));
}

#[test]
fn fix_deletes_the_line_and_the_rerun_is_clean() {
    let tree = tree_with_unused_pragma();
    rustmanifest()
        .arg("check")
        .arg("--resolution")
        .arg(settled_overlay(&tree))
        .arg("--fix")
        .arg("--format")
        .arg("json")
        .arg(tree.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("[]"));
    let text = fs::read_to_string(tree.path().join("src/lib.rs")).unwrap();
    assert!(!text.contains("rustmanifest: allow"));
    assert!(text.contains("pub fn plain"));
    rustmanifest()
        .arg("check")
        .arg("--resolution")
        .arg(settled_overlay(&tree))
        .arg(tree.path())
        .assert()
        .success();
}

fn tree_with_hoistable_clone() -> TempDir {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(
        dir.path().join("src/lib.rs"),
        "#[derive(Clone)]\npub struct Config {\n    pub retries: u32\n}\n\npub fn total(config: \
         &Config, jobs: &[u32]) -> u32 {\n    let mut total = 0;\n    for job in jobs {\n        \
         let snapshot = config.clone();\n        total += snapshot.retries + job;\n    }\n    \
         total\n}\n"
    )
    .unwrap();
    dir
}

#[test]
fn hoist_fix_moves_the_clone_before_the_loop_and_rerun_is_clean() {
    let tree = tree_with_hoistable_clone();
    rustmanifest()
        .arg("check")
        .arg("--resolution")
        .arg(settled_overlay(&tree))
        .arg("--format")
        .arg("json")
        .arg(tree.path())
        .assert()
        .failure()
        .stdout(predicate::str::contains("RM-MEM-002"));
    rustmanifest()
        .arg("check")
        .arg("--resolution")
        .arg(settled_overlay(&tree))
        .arg("--fix")
        .arg("--format")
        .arg("json")
        .arg(tree.path())
        .assert()
        .success();
    let text = fs::read_to_string(tree.path().join("src/lib.rs")).unwrap();
    let clone_at = text.find("let snapshot = config.clone();").unwrap();
    let loop_at = text.find("for job in jobs").unwrap();
    assert!(
        clone_at < loop_at,
        "clone must sit before the loop:\n{text}"
    );
    rustmanifest()
        .arg("check")
        .arg("--resolution")
        .arg(settled_overlay(&tree))
        .arg(tree.path())
        .assert()
        .success();
}

fn tree_with_borrowable_param() -> TempDir {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(
        dir.path().join("src/lib.rs"),
        "pub struct Report {\n    pub total: u64\n}\n\nfn summarize(report: Report) -> u64 {\n    \
         report.total\n}\n\npub fn run(report: Report) -> u64 {\n    summarize(report)\n}\n"
    )
    .unwrap();
    dir
}

#[test]
fn borrow_fix_rewrites_signature_and_caller_and_rerun_is_clean() {
    let tree = tree_with_borrowable_param();
    rustmanifest()
        .arg("check")
        .arg("--resolution")
        .arg(settled_overlay(&tree))
        .arg("--fix")
        .arg("--format")
        .arg("json")
        .arg(tree.path())
        .assert()
        .failure();
    let text = fs::read_to_string(tree.path().join("src/lib.rs")).unwrap();
    assert!(text.contains("fn summarize(report: &Report)"), "{text}");
    assert!(text.contains("summarize(&report)"), "{text}");
    rustmanifest()
        .arg("check")
        .arg("--resolution")
        .arg(settled_overlay(&tree))
        .arg(tree.path())
        .assert()
        .success();
}

#[test]
fn borrow_fix_rewrites_a_caller_in_another_file() {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(
        dir.path().join("src/lib.rs"),
        "mod caller;\npub struct Report {\n    pub total: u64\n}\n\npub(crate) fn \
         summarize(report: Report) -> u64 {\n    report.total\n}\n"
    )
    .unwrap();
    fs::write(
        dir.path().join("src/caller.rs"),
        "use crate::{Report, summarize};\n\npub fn run(report: Report) -> u64 {\n    \
         summarize(report)\n}\n"
    )
    .unwrap();
    rustmanifest()
        .arg("check")
        .arg("--resolution")
        .arg(settled_overlay(&dir))
        .arg("--fix")
        .arg("--format")
        .arg("json")
        .arg(dir.path())
        .assert()
        .failure();
    let lib = fs::read_to_string(dir.path().join("src/lib.rs")).unwrap();
    let caller = fs::read_to_string(dir.path().join("src/caller.rs")).unwrap();
    assert!(lib.contains("summarize(report: &Report)"), "{lib}");
    assert!(caller.contains("summarize(&report)"), "{caller}");
}
