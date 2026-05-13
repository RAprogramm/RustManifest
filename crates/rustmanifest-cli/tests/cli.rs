// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Integration tests for the `rustmanifest` CLI binary.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]

use std::{fs, io::Write};

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn rustmanifest() -> Command {
    Command::cargo_bin("rustmanifest").unwrap()
}

fn make_tree(files: &[(&str, &str)]) -> TempDir {
    let dir = TempDir::new().unwrap();
    for (name, contents) in files {
        let path = dir.path().join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut handle = fs::File::create(&path).unwrap();
        handle.write_all(contents.as_bytes()).unwrap();
    }
    dir
}

#[test]
fn version_works() {
    rustmanifest()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("rustmanifest"));
}

#[test]
fn check_clean_tree_exits_zero() {
    let tree = make_tree(&[("src/clean.rs", "fn main() { println!(\"hi\"); }\n")]);
    rustmanifest()
        .arg("check")
        .arg(tree.path())
        .arg("--format")
        .arg("json")
        .assert()
        .success();
}

#[test]
fn check_dirty_tree_exits_one_and_emits_json() {
    let tree = make_tree(&[("src/dirty.rs", "let password = \"hunter2\";\n")]);
    let assert = rustmanifest()
        .arg("check")
        .arg(tree.path())
        .arg("--format")
        .arg("json")
        .assert()
        .failure()
        .code(1);
    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("RM-SEC-001"),
        "expected RM-SEC-001 in JSON output, got: {stdout}"
    );
}

#[test]
fn severity_filter_warning_skips_warning_rules_when_set_to_error() {
    let tree = make_tree(&[("src/vec.rs", "let v: Vec<u8> = Vec::new();\n")]);
    rustmanifest()
        .arg("check")
        .arg(tree.path())
        .arg("--format")
        .arg("json")
        .arg("--severity-filter")
        .arg("error")
        .assert()
        .success();
}

#[test]
fn sarif_output_is_well_formed() {
    let tree = make_tree(&[("src/dirty.rs", "let password = \"hunter2\";\n")]);
    let assert = rustmanifest()
        .arg("check")
        .arg(tree.path())
        .arg("--format")
        .arg("sarif")
        .assert()
        .failure()
        .code(1);
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("SARIF output must be valid JSON");
    assert_eq!(parsed["version"], "2.1.0");
    assert!(
        parsed["runs"][0]["tool"]["driver"]["name"]
            .as_str()
            .is_some()
    );
}

#[test]
fn pragma_suppresses_finding() {
    let tree = make_tree(&[(
        "src/suppressed.rs",
        "// rustmanifest: allow(RM-SEC-001) reason=\"fixture\"\nlet password = \"hunter2\";\n"
    )]);
    rustmanifest()
        .arg("check")
        .arg(tree.path())
        .arg("--format")
        .arg("json")
        .assert()
        .success();
}
