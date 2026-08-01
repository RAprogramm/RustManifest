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
        .arg(tree.path())
        .assert()
        .success();
}
