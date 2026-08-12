// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Producing the resolution from the command line.
//!
//! Every tier that answers about types, modules, calls or signatures reads
//! this overlay and is silent without one. Producing it used to mean
//! knowing that a second binary exists.

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

/// A package whose one type carries one method, so the resolution has a
/// signature, a module and a body to state.
fn probe() -> TempDir {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"probe\"\nversion = \"0.0.0\"\nedition = \"2024\"\n"
    )
    .unwrap();
    fs::write(
        dir.path().join("src/lib.rs"),
        "pub struct Store {\n    pub count: u64\n}\n\
         impl Store {\n    pub fn peek(&self) -> u64 {\n        self.count\n    }\n}\n"
    )
    .unwrap();
    dir
}

#[test]
fn resolving_a_workspace_writes_what_the_compiler_settled() {
    let dir = probe();
    rustmanifest()
        .arg("resolve")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("resolution written"));

    let text = fs::read_to_string(dir.path().join("resolution.json")).unwrap();
    let settled: rustmanifest_schema::Resolution = serde_json::from_str(&text).unwrap();
    assert!(
        settled
            .functions
            .iter()
            .any(|held| held.names.ends_with("Store::peek")),
        "the declared body is missing: {:?}",
        settled.functions
    );
    assert!(
        settled.signatures.iter().any(|held| held.returns == "u64"),
        "the signature is missing: {:?}",
        settled.signatures
    );
}

#[test]
fn a_path_that_is_no_workspace_reports_and_writes_nothing() {
    let dir = TempDir::new().unwrap();
    rustmanifest()
        .arg("resolve")
        .arg(dir.path())
        .assert()
        .code(2)
        .stdout(predicate::str::contains("not produced"));
    assert!(!dir.path().join("resolution.json").exists());
}
