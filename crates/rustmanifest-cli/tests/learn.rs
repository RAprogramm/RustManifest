// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Integration tests for the `learn` subcommand.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]

use std::{fmt::Write, fs};

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn rustmanifest() -> Command {
    Command::cargo_bin("rustmanifest").unwrap()
}

fn mixed_tree() -> TempDir {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    let mut text = String::new();
    for index in 0..25 {
        writeln!(
            text,
            "pub fn log_{index}(text: &str) {{ println!(\"{{text}}\"); }}"
        )
        .unwrap();
        writeln!(
            text,
            "pub fn calc_{index}(value: u64) -> u64 {{ value * 2 }}"
        )
        .unwrap();
    }
    fs::write(dir.path().join("src/lib.rs"), text).unwrap();
    dir
}

#[test]
fn learn_emits_role_clauses_as_json() {
    rustmanifest()
        .arg("learn")
        .arg(mixed_tree().path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"role\": \"io\""))
        .stdout(predicate::str::contains("\"if\""));
}

#[test]
fn learn_is_deterministic_across_runs() {
    let tree = mixed_tree();
    let first = rustmanifest()
        .arg("learn")
        .arg(tree.path())
        .output()
        .unwrap()
        .stdout;
    let second = rustmanifest()
        .arg("learn")
        .arg(tree.path())
        .output()
        .unwrap()
        .stdout;
    assert_eq!(first, second);
}
