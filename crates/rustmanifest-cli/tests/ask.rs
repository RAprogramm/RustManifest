// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Integration tests for the `ask` subcommand's exit discipline: an
//! answer and a refusal must be distinguishable by a script, not only
//! by a reader.

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

fn tree() -> TempDir {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(
        dir.path().join("src/lib.rs"),
        "pub fn report(text: &str) {\n    println!(\"{text}\");\n}\n"
    )
    .unwrap();
    dir
}

#[test]
fn an_answered_question_exits_clean_with_its_steps() {
    let bed = tree();
    rustmanifest()
        .arg("ask")
        .arg("report")
        .arg("console")
        .arg(bed.path())
        .assert()
        .code(0)
        .stdout(predicate::str::contains("=> "))
        .stdout(predicate::str::contains("console"));
}

#[test]
fn a_refused_question_exits_distinctly() {
    let bed = tree();
    rustmanifest()
        .arg("ask")
        .arg("nowhere")
        .arg("console")
        .arg(bed.path())
        .assert()
        .code(3)
        .stdout(predicate::str::contains(
            "no analyzed function is named `nowhere`"
        ));
}
