// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Integration tests for the `--delegate-clippy` flag of `rustmanifest check`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    reason = "test code is allowed to fail loudly and to note skipped externals"
)]

use std::{fs, path::Path, process::Output};

use assert_cmd::Command;
use tempfile::TempDir;

fn rustmanifest() -> Command {
    Command::cargo_bin("rustmanifest").unwrap()
}

/// Returns `false` when `cargo clippy` cannot run in this environment, in
/// which case delegation tests skip gracefully with a note on stderr.
fn cargo_clippy_available() -> bool {
    let probe = std::process::Command::new("cargo")
        .args(["clippy", "--version"])
        .output();
    match probe {
        Ok(output) if output.status.success() => true,
        _ => {
            eprintln!("skipping test: cargo clippy is unavailable in this environment");
            false
        }
    }
}

/// Builds a minimal cargo package whose only source file triggers the
/// `clippy::clone_on_copy` lint deterministically.
fn make_clippy_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"delegate-fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n"
    )
    .unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(
        dir.path().join("src").join("main.rs"),
        "fn main() {\n    let x = 5_i32;\n    let _y = x.clone();\n}\n"
    )
    .unwrap();
    dir
}

fn run_check(path: &Path, delegate: bool) -> Output {
    let mut command = rustmanifest();
    command.arg("check").arg(path).arg("--format").arg("json");
    if delegate {
        command.arg("--delegate-clippy");
    }
    command.output().unwrap()
}

fn parse_findings(output: &Output) -> Vec<serde_json::Value> {
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str::<serde_json::Value>(&stdout)
        .expect("JSON output must parse")
        .as_array()
        .expect("JSON output must be an array")
        .clone()
}

#[test]
fn delegate_clippy_surfaces_clippy_findings_with_rationale() {
    if !cargo_clippy_available() {
        return;
    }
    let project = make_clippy_project();
    let output = run_check(project.path(), true);
    assert_ne!(
        output.status.code(),
        Some(2),
        "delegated run must not fail operationally: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let findings = parse_findings(&output);
    let delegated = findings
        .iter()
        .find(|finding| {
            finding["rule-id"]
                .as_str()
                .is_some_and(|id| id.starts_with("clippy::"))
        })
        .expect("expected at least one delegated clippy finding");
    let rationale = delegated["rationale-uri"].as_str().unwrap();
    assert!(
        rationale.starts_with("https://rust-lang.github.io/rust-clippy/master/index.html#"),
        "rationale must point at the clippy index, got: {rationale}"
    );
}

#[test]
fn without_flag_no_clippy_findings_are_emitted() {
    if !cargo_clippy_available() {
        return;
    }
    let project = make_clippy_project();
    let output = run_check(project.path(), false);
    assert_ne!(output.status.code(), Some(2));
    let findings = parse_findings(&output);
    assert!(
        findings.iter().all(|finding| {
            finding["rule-id"]
                .as_str()
                .is_none_or(|id| !id.starts_with("clippy::"))
        }),
        "no clippy findings expected without --delegate-clippy"
    );
}

#[test]
fn delegate_clippy_without_manifest_warns_and_succeeds() {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(
        dir.path().join("src").join("clean.rs"),
        "pub fn add(left: u32, right: u32) -> u32 {\n    left + right\n}\n"
    )
    .unwrap();
    let output = run_check(dir.path(), true);
    assert_eq!(
        output.status.code(),
        Some(0),
        "run must succeed without delegated findings: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("no Cargo.toml"),
        "expected a warning about the missing manifest, got: {stderr}"
    );
    let findings = parse_findings(&output);
    assert!(
        findings.iter().all(|finding| {
            finding["rule-id"]
                .as_str()
                .is_none_or(|id| !id.starts_with("clippy::"))
        }),
        "no delegated findings expected without a manifest"
    );
}
