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

/// Writes beside `tree` what a compiler settles about its sources, and
/// answers with the path the run reads it from.
fn settled_overlay(tree: &TempDir) -> std::path::PathBuf {
    let files = rustmanifest_engine::walker::discover(&[tree.path().to_path_buf()])
        .expect("the tree is walkable");
    let sources: Vec<rustmanifest_engine::Source> = files
        .iter()
        .filter_map(|path| {
            let text = fs::read_to_string(path).ok()?;
            let named = path.strip_prefix(tree.path()).unwrap_or(path);
            Some(rustmanifest_engine::Source::new(named.to_path_buf(), text))
        })
        .collect();
    let settled = settled_over(&sources);
    let at = tree.path().join("resolution.json");
    fs::write(
        &at,
        serde_json::to_string(&settled).expect("the overlay serializes")
    )
    .expect("the overlay is written");
    at
}

/// What a compiler settles about `sources`, asked of the compiler.
fn settled_over(sources: &[rustmanifest_engine::Source]) -> rustmanifest_schema::Resolution {
    let written: Vec<(String, String)> = sources
        .iter()
        .map(|held| (held.path().display().to_string(), held.text().to_owned()))
        .collect();
    let asked: Vec<(&str, &str)> = written
        .iter()
        .map(|(path, text)| (path.as_str(), text.as_str()))
        .collect();
    rustmanifest_fixture::settled(&asked)
}

#[test]
fn learn_emits_role_clauses_as_json() {
    let tree = mixed_tree();
    rustmanifest()
        .arg("learn")
        .arg(tree.path())
        .arg("--resolution")
        .arg(settled_overlay(&tree))
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
