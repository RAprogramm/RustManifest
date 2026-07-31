// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Integration tests for user-declared invariants in `check`.

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

fn tree_with_invariant() -> TempDir {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(
        dir.path().join("src/lib.rs"),
        "mod ledger;\nmod api;\npub struct Wallet { pub balance: i64 }\n"
    )
    .unwrap();
    fs::write(
        dir.path().join("src/ledger.rs"),
        "use crate::Wallet;\npub fn post(wallet: &mut Wallet, amount: i64) { wallet.balance += \
         amount; }\n"
    )
    .unwrap();
    fs::write(
        dir.path().join("src/api.rs"),
        "use crate::Wallet;\npub fn hack(wallet: &mut Wallet) { wallet.balance = 0; }\n"
    )
    .unwrap();
    fs::write(
        dir.path().join("rustmanifest.toml"),
        "[[invariants]]\nid = \"WALLET-BALANCE\"\nfield = \
         \"Wallet.balance\"\nwrite-only-from = [\"crate::ledger\"]\n"
    )
    .unwrap();
    dir
}

#[test]
fn invariant_violation_is_reported_and_fails_the_run() {
    let tree = tree_with_invariant();
    rustmanifest()
        .arg("check")
        .arg("--config")
        .arg(tree.path().join("rustmanifest.toml"))
        .arg("--format")
        .arg("json")
        .arg(tree.path())
        .assert()
        .code(1)
        .stdout(predicate::str::contains("WALLET-BALANCE"))
        .stdout(predicate::str::contains(
            "`hack` writes `Wallet.balance` from module `crate::api`"
        ))
        .stdout(predicate::str::contains(
            "rustmanifest://invariants/WALLET-BALANCE"
        ));
}

#[test]
fn allowed_writer_produces_no_invariant_finding() {
    let tree = tree_with_invariant();
    let output = rustmanifest()
        .arg("check")
        .arg("--config")
        .arg(tree.path().join("rustmanifest.toml"))
        .arg("--format")
        .arg("json")
        .arg(tree.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("`hack` writes"));
    assert!(!stdout.contains("`post` writes"));
}
