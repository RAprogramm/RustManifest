// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Producing the compiler's documentation index from the command line.
//!
//! One of the two tests here needs a nightly toolchain, because the emitter
//! is unstable. It states that and skips rather than failing, so a machine
//! without one still runs the suite — but it never passes by pretending:
//! the skip is printed.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    reason = "test code is allowed to fail loudly on broken invariants, and a skipped test must \
              say so where a runner shows it"
)]

use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn rustmanifest() -> Command {
    Command::cargo_bin("rustmanifest").unwrap()
}

/// A crate declaring one type with exactly one written implementation.
/// Every other implementation the index lists is the compiler's own.
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
        "pub struct Wallet {\n    pub balance: i64\n}\n\
         impl std::fmt::Display for Wallet {\n    \
         fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {\n        \
         write!(f, \"{}\", self.balance)\n    }\n}\n"
    )
    .unwrap();
    dir
}

/// Whether a nightly toolchain is installed, since the emitter is unstable.
fn nightly_installed() -> bool {
    std::process::Command::new("cargo")
        .args(["+nightly", "--version"])
        .output()
        .is_ok_and(|output| output.status.success())
}

#[test]
fn documenting_a_path_that_is_no_package_says_so_and_writes_nothing() {
    let dir = TempDir::new().unwrap();
    rustmanifest()
        .arg("document")
        .arg(dir.path())
        .assert()
        .code(2);
    assert!(!dir.path().join("target").join("doc").exists());
}

#[test]
fn the_index_lists_what_the_compiler_grants_and_not_only_what_was_written() {
    if !nightly_installed() {
        eprintln!("skipped: the documentation emitter needs a nightly toolchain");
        return;
    }
    let dir = probe();
    rustmanifest()
        .arg("document")
        .arg(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("emission(s)"));

    let emitted = fs::read_dir(dir.path().join("target").join("doc"))
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .find(|path| path.extension().is_some_and(|held| held == "json"))
        .expect("the emitter wrote an index");
    let text = fs::read_to_string(emitted).unwrap();
    assert!(
        text.contains("\"Display\""),
        "the written implementation is missing from the index"
    );
    assert!(
        text.contains("\"Sync\"") && text.contains("\"is_synthetic\":true"),
        "an auto trait the compiler grants is missing from the index"
    );
}
