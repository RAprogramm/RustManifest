// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Integration tests for the `coverage` subcommand.

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
fn coverage_reports_parsed_files_and_exits_clean() {
    let tree = make_tree(&[
        ("src/lib.rs", "mod util;\n"),
        (
            "src/util.rs",
            "#[derive(Debug)]\npub struct Item {\n    pub id: u64\n}\n"
        )
    ]);
    rustmanifest()
        .arg("coverage")
        .arg(tree.path())
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "files parsed        2/2       100.0%"
        ))
        .stdout(predicate::str::contains("modules mapped      0"))
        .stdout(predicate::str::contains("structs captured    1"))
        .stdout(predicate::str::contains("(expansions not analyzed)"));
}

#[test]
fn coverage_counts_unparsable_files_honestly() {
    let tree = make_tree(&[
        ("src/lib.rs", "pub struct Ok;\n"),
        ("src/broken.rs", "fn broken(\n")
    ]);
    rustmanifest()
        .arg("coverage")
        .arg(tree.path())
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "files parsed        1/2       50.0%"
        ));
}
