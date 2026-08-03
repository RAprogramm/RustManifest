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
    let tree = make_tree(&[(
        "src/clean.rs",
        "pub fn add(left: u32, right: u32) -> u32 {\n    left + right\n}\n"
    )]);
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
fn explain_known_rule_prints_title_and_uri() {
    rustmanifest()
        .arg("explain")
        .arg("RM-SEC-001")
        .assert()
        .success()
        .stdout(predicate::str::contains("RM-SEC-001"))
        .stdout(predicate::str::contains("Hardcoded credentials"))
        .stdout(predicate::str::contains(
            "rustmanifest://methodology/security-vulnerabilities"
        ));
}

#[test]
fn explain_unknown_rule_exits_two_and_lists_known_ids() {
    let assert = rustmanifest()
        .arg("explain")
        .arg("RM-NOPE-999")
        .assert()
        .failure()
        .code(2);
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr);
    assert!(
        stderr.contains("RM-SEC-001"),
        "expected known rule ids in error output, got: {stderr}"
    );
}

#[test]
fn config_override_changes_finding_severity_in_json() {
    let tree = make_tree(&[
        ("src/dirty.rs", "let password = \"hunter2\";\n"),
        (
            "rustmanifest.toml",
            "[[overrides]]\nrule-id = \"RM-SEC-001\"\nseverity = \"warning\"\n"
        )
    ]);
    let assert = rustmanifest()
        .arg("check")
        .arg(tree.path().join("src"))
        .arg("--config")
        .arg(tree.path().join("rustmanifest.toml"))
        .arg("--format")
        .arg("json")
        .assert()
        .failure()
        .code(1);
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    let findings: serde_json::Value =
        serde_json::from_str(&stdout).expect("JSON output must parse");
    let overridden = findings
        .as_array()
        .expect("JSON output must be an array")
        .iter()
        .find(|finding| finding["rule-id"] == "RM-SEC-001")
        .expect("RM-SEC-001 finding must be present");
    assert_eq!(overridden["severity"], "warning");
}

#[test]
fn config_exclude_glob_removes_findings() {
    let tree = make_tree(&[
        ("src/dirty.rs", "let password = \"hunter2\";\n"),
        ("rustmanifest.toml", "exclude = [\"**/dirty.rs\"]\n")
    ]);
    rustmanifest()
        .arg("check")
        .arg(tree.path().join("src"))
        .arg("--config")
        .arg(tree.path().join("rustmanifest.toml"))
        .arg("--format")
        .arg("json")
        .assert()
        .success();
}

#[test]
fn config_minimal_profile_skips_non_error_rules() {
    let tree = make_tree(&[
        ("src/vec.rs", "let v: Vec<u8> = Vec::new();\n"),
        ("rustmanifest.toml", "profile = \"minimal\"\n")
    ]);
    rustmanifest()
        .arg("check")
        .arg(tree.path().join("src"))
        .arg("--config")
        .arg(tree.path().join("rustmanifest.toml"))
        .arg("--format")
        .arg("json")
        .assert()
        .success();
}

#[test]
fn config_unknown_profile_exits_two() {
    let tree = make_tree(&[
        ("src/clean.rs", "fn main() {}\n"),
        ("rustmanifest.toml", "profile = \"paranoid\"\n")
    ]);
    rustmanifest()
        .arg("check")
        .arg(tree.path().join("src"))
        .arg("--config")
        .arg(tree.path().join("rustmanifest.toml"))
        .assert()
        .failure()
        .code(2);
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

#[test]
fn cache_second_run_produces_identical_json_output() {
    let tree = make_tree(&[
        ("src/dirty.rs", "let password = \"hunter2\";\n"),
        (
            "src/stale.rs",
            "// rustmanifest: allow(RM-SEC-001) reason=\"stale\"\nfn main() {}\n"
        )
    ]);
    let cache_dir = TempDir::new().unwrap();

    let run = || {
        let assert = rustmanifest()
            .arg("check")
            .arg(tree.path())
            .arg("--cache")
            .arg(cache_dir.path())
            .arg("--format")
            .arg("json")
            .assert()
            .failure()
            .code(1);
        assert.get_output().stdout.clone()
    };

    let first = run();
    let entry_count = fs::read_dir(cache_dir.path()).unwrap().count();
    assert!(entry_count > 0, "expected cache entries after first run");
    let second = run();
    assert_eq!(first, second, "cached run must match the fresh run exactly");

    let findings: serde_json::Value = serde_json::from_slice(&second).unwrap();
    let rendered = serde_json::to_string(&findings).unwrap();
    assert!(rendered.contains("RM-SEC-001"));
    assert!(rendered.contains("RM-META-001"));
}

#[test]
fn cache_creates_missing_directory() {
    let tree = make_tree(&[(
        "src/clean.rs",
        "pub fn add(left: u32, right: u32) -> u32 {\n    left + right\n}\n"
    )]);
    let parent = TempDir::new().unwrap();
    let cache_dir = parent.path().join("nested").join("cache");

    rustmanifest()
        .arg("check")
        .arg(tree.path())
        .arg("--cache")
        .arg(&cache_dir)
        .arg("--format")
        .arg("json")
        .assert()
        .success();
    assert!(
        cache_dir.is_dir(),
        "cache directory should have been created"
    );
}
