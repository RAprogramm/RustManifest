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

fn wallet_tree() -> TempDir {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(
        dir.path().join("src/lib.rs"),
        "pub struct Wallet { pub balance: i64 }\nimpl Wallet {\n    pub fn credit(&mut self, \
         amount: i64) -> Result<(), String> {\n        self.balance += amount;\n        \
         Ok(())\n    }\n    pub fn balance(&self) -> i64 { self.balance }\n}\n"
    )
    .unwrap();
    dir
}

#[test]
fn facts_report_writes_reads_and_error_types() {
    rustmanifest()
        .arg("facts")
        .arg(wallet_tree().path())
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
