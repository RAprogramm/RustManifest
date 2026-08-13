// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Integration tests for the `facts` subcommand.

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

/// The wallet crate, with what the compiler settles about it written
/// beside it.
///
/// A fact about a field names the type holding it, and which type that is
/// is the compiler's answer. The tree is a cargo crate so the resolver can
/// read it, and the run is handed what it wrote.
fn wallet_tree() -> TempDir {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"wallet\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[dependencies]\n"
    )
    .unwrap();
    fs::write(
        dir.path().join("src/lib.rs"),
        "pub struct Wallet { pub balance: i64 }\nimpl Wallet {\n    pub fn credit(&mut self, \
         amount: i64) -> Result<(), String> {\n        self.balance += amount;\n        \
         Ok(())\n    }\n    pub fn balance(&self) -> i64 { self.balance }\n}\n"
    )
    .unwrap();
    Command::cargo_bin("rustmanifest-resolve")
        .unwrap()
        .arg(dir.path())
        .arg(dir.path().join("resolution.json"))
        .assert()
        .success();
    dir
}

#[test]
fn facts_report_writes_reads_and_error_types() {
    let tree = wallet_tree();
    rustmanifest()
        .arg("facts")
        .arg("--resolution")
        .arg(tree.path().join("resolution.json"))
        .arg(tree.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"function\": \"Wallet::credit\""))
        .stdout(predicate::str::contains("Wallet.balance"))
        .stdout(predicate::str::contains("\"String\""));
}

#[test]
fn active_filter_drops_pure_readers() {
    rustmanifest()
        .arg("facts")
        .arg("--active")
        .arg(wallet_tree().path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Wallet::credit"))
        .stdout(predicate::str::contains("Wallet::balance\"").not());
}

#[test]
fn name_filter_selects_matching_functions() {
    rustmanifest()
        .arg("facts")
        .arg("--filter")
        .arg("credit")
        .arg(wallet_tree().path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Wallet::credit"))
        .stdout(predicate::str::contains("Wallet::balance\"").not());
}
