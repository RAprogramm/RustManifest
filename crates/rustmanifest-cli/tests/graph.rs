// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Integration tests for the `graph` subcommand.

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
fn modules_mermaid_contains_header_nodes_and_edge() {
    let tree = make_tree(&[
        ("src/lib.rs", "mod util;\n"),
        ("src/util.rs", "use crate::Item;\n")
    ]);
    rustmanifest()
        .arg("graph")
        .arg(tree.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("graph LR"))
        .stdout(predicate::str::contains("crate_util[\"crate::util\"]"))
        .stdout(predicate::str::contains("crate_util --> crate"));
}

#[test]
fn modules_mermaid_styles_cycle_members() {
    let tree = make_tree(&[(
        "src/lib.rs",
        "mod a { use crate::b::Item; }\nmod b { use crate::a::Other; }\n"
    )]);
    rustmanifest()
        .arg("graph")
        .arg(tree.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("classDef cycle"))
        .stdout(predicate::str::contains("crate_a[\"crate::a\"]:::cycle"))
        .stdout(predicate::str::contains("crate_b[\"crate::b\"]:::cycle"));
}

#[test]
fn modules_dot_marks_cycle_members_red() {
    let tree = make_tree(&[(
        "src/lib.rs",
        "mod a { use crate::b::Item; }\nmod b { use crate::a::Other; }\n"
    )]);
    rustmanifest()
        .arg("graph")
        .arg(tree.path())
        .arg("--format")
        .arg("dot")
        .assert()
        .success()
        .stdout(predicate::str::contains("digraph modules {"))
        .stdout(predicate::str::contains("\"crate::a\" [color=\"red\"];"))
        .stdout(predicate::str::contains("\"crate::a\" -> \"crate::b\";"));
}

#[test]
fn types_dot_contains_aggregation_edge() {
    let tree = make_tree(&[
        ("src/identity.rs", "pub struct Identity { pub id: u64 }\n"),
        (
            "src/user.rs",
            "pub struct User { pub identity: Identity }\n"
        )
    ]);
    rustmanifest()
        .arg("graph")
        .arg(tree.path())
        .arg("--kind")
        .arg("types")
        .arg("--format")
        .arg("dot")
        .assert()
        .success()
        .stdout(predicate::str::contains("digraph types {"))
        .stdout(predicate::str::contains(
            "\"User\" -> \"Identity\" [label=\"identity\"];"
        ));
}

#[test]
fn types_mermaid_labels_edges_with_field_names() {
    let tree = make_tree(&[
        ("src/identity.rs", "pub struct Identity { pub id: u64 }\n"),
        (
            "src/user.rs",
            "pub struct User { pub identity: Identity }\n"
        )
    ]);
    rustmanifest()
        .arg("graph")
        .arg(tree.path())
        .arg("--kind")
        .arg("types")
        .assert()
        .success()
        .stdout(predicate::str::contains("graph LR"))
        .stdout(predicate::str::contains("User -->|identity| Identity"));
}

#[test]
fn unknown_kind_is_rejected() {
    rustmanifest()
        .arg("graph")
        .arg(".")
        .arg("--kind")
        .arg("bogus")
        .assert()
        .failure()
        .code(2);
}

#[test]
fn unknown_format_is_rejected() {
    rustmanifest()
        .arg("graph")
        .arg(".")
        .arg("--format")
        .arg("bogus")
        .assert()
        .failure()
        .code(2);
}
