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

/// Writes the module tree a resolver would state for the fixture.
fn overlay_with(
    dir: &std::path::Path,
    rows: &[(&str, &str)],
    fields: &[(&str, u32, u32, &str, &str)]
) {
    let modules: Vec<String> = rows
        .iter()
        .map(|(names, file)| {
            let root = !names.contains("::");
            let parent = names.rsplit_once("::").map_or(String::new(), |(above, _)| {
                format!(", \"parent\": \"{above}\"")
            });
            format!(
                "{{\"names\": \"{names}\", \"file\": \"{file}\", \"crate-root\": {root}{parent}}}"
            )
        })
        .collect();
    let held: Vec<String> = fields
        .iter()
        .map(|(file, start, end, owner, produced)| {
            format!(
                "{{\"at\": {{\"file\": \"{file}\", \"range\": {{\"start\": {start}, \
                 \"end\": {end}}}}}, \"owner\": \"{owner}\", \"produced\": \
                 \"{produced}\"}}"
            )
        })
        .collect();
    fs::write(
        dir.join("resolution.json"),
        format!(
            "{{\"resolver\": \"test\", \"calls\": [], \"fields\": [{}], \"modules\": [{}]}}",
            held.join(", "),
            modules.join(", ")
        )
    )
    .unwrap();
}

fn overlay(dir: &std::path::Path, rows: &[(&str, &str)]) {
    let modules: Vec<String> = rows
        .iter()
        .map(|(names, file)| {
            let root = !names.contains("::");
            let parent = names.rsplit_once("::").map_or(String::new(), |(above, _)| {
                format!(", \"parent\": \"{above}\"")
            });
            format!(
                "{{\"names\": \"{names}\", \"file\": \"{file}\", \"crate-root\": {root}{parent}}}"
            )
        })
        .collect();
    fs::write(
        dir.join("resolution.json"),
        format!(
            "{{\"resolver\": \"test\", \"calls\": [], \"modules\": [{}]}}",
            modules.join(", ")
        )
    )
    .unwrap();
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
    overlay_with(
        dir.path(),
        &[
            ("fixture", "src/lib.rs"),
            ("fixture::ledger", "src/ledger.rs"),
            ("fixture::api", "src/api.rs")
        ],
        &[
            ("src/ledger.rs", 67, 81, "&mut Wallet", "i64"),
            ("src/api.rs", 54, 68, "&mut Wallet", "i64")
        ]
    );
    dir
}

#[test]
fn invariant_violation_is_reported_and_fails_the_run() {
    let tree = tree_with_invariant();
    rustmanifest()
        .current_dir(tree.path())
        .arg("check")
        .arg("--resolution")
        .arg("resolution.json")
        .arg("--config")
        .arg("rustmanifest.toml")
        .arg("--format")
        .arg("json")
        .arg(".")
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

/// Encodes one base-128 varint, the only integer shape SCIP uses.
fn put_varint(out: &mut Vec<u8>, mut value: u64) {
    loop {
        let byte = u8::try_from(value & 0x7f).unwrap();
        value >>= 7;
        if value == 0 {
            out.push(byte);
            break;
        }
        out.push(byte | 0x80);
    }
}

/// Appends one length-delimited protobuf field.
fn put_field(out: &mut Vec<u8>, field: u64, payload: &[u8]) {
    put_varint(out, (field << 3) | 2);
    put_varint(out, payload.len() as u64);
    out.extend_from_slice(payload);
}

/// Encodes one SCIP occurrence: packed range, symbol, roles, enclosing
/// range.
fn occurrence(symbol: &str, range: &[i32], definition: bool, enclosing: &[i32]) -> Vec<u8> {
    let packed = |values: &[i32]| {
        let mut out = Vec::new();
        for value in values {
            put_varint(&mut out, u64::from(u32::try_from(*value).unwrap()));
        }
        out
    };
    let mut out = Vec::new();
    put_field(&mut out, 1, &packed(range));
    put_field(&mut out, 2, symbol.as_bytes());
    if definition {
        put_varint(&mut out, 3 << 3);
        put_varint(&mut out, 1);
    }
    if !enclosing.is_empty() {
        put_field(&mut out, 7, &packed(enclosing));
    }
    out
}

/// A tree whose only path from the denied module to the network effect
/// runs through a macro-wrapped call, plus a SCIP index that resolves it.
fn tree_with_indexed_effect() -> TempDir {
    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/lib.rs"), "mod domain;\nmod net;\n").unwrap();
    fs::write(
        dir.path().join("src/net.rs"),
        "pub fn dial() { std::net::TcpStream::connect(\"h\").ok(); }\n"
    )
    .unwrap();
    fs::write(
        dir.path().join("src/domain.rs"),
        "pub fn drive() { dispatch!(dial()); }\n"
    )
    .unwrap();
    fs::write(
        dir.path().join("rustmanifest.toml"),
        "[[invariants]]\nid = \"NO-NET\"\neffect = \"net\"\ndeny-in = [\"crate::domain\"]\n"
    )
    .unwrap();
    let mut document = Vec::new();
    put_field(&mut document, 1, b"src/domain.rs");
    put_field(
        &mut document,
        2,
        &occurrence(
            "rust-analyzer cargo demo 0.1.0 domain/drive().",
            &[0, 7, 12],
            true,
            &[0, 0, 0, 36]
        )
    );
    put_field(
        &mut document,
        2,
        &occurrence(
            "rust-analyzer cargo demo 0.1.0 net/dial().",
            &[0, 26, 30],
            false,
            &[]
        )
    );
    let mut index = Vec::new();
    put_field(&mut index, 2, &document);
    fs::write(dir.path().join("index.scip"), index).unwrap();
    overlay(
        dir.path(),
        &[
            ("demo", "src/lib.rs"),
            ("demo::net", "src/net.rs"),
            ("demo::domain", "src/domain.rs")
        ]
    );
    dir
}

#[test]
fn an_effect_ban_stays_silent_without_the_index() {
    let tree = tree_with_indexed_effect();
    let output = rustmanifest()
        .current_dir(tree.path())
        .arg("check")
        .arg("--resolution")
        .arg("resolution.json")
        .arg("--config")
        .arg("rustmanifest.toml")
        .arg("--format")
        .arg("json")
        .arg(".")
        .output()
        .unwrap();
    assert!(!String::from_utf8(output.stdout).unwrap().contains("NO-NET"));
}

#[test]
fn the_index_widens_the_graph_the_ban_is_judged_against() {
    let tree = tree_with_indexed_effect();
    rustmanifest()
        .current_dir(tree.path())
        .arg("check")
        .arg("--resolution")
        .arg("resolution.json")
        .arg("--config")
        .arg("rustmanifest.toml")
        .arg("--scip")
        .arg("index.scip")
        .arg("--format")
        .arg("json")
        .arg(".")
        .assert()
        .code(1)
        .stdout(predicate::str::contains("NO-NET"))
        .stdout(predicate::str::contains(
            "`drive` transitively performs `net`"
        ))
        .stdout(predicate::str::contains(
            "name-resolved call edge, read from the SCIP index"
        ));
}

#[test]
fn a_missing_index_fails_the_run_with_context() {
    let tree = tree_with_indexed_effect();
    rustmanifest()
        .current_dir(tree.path())
        .arg("check")
        .arg("--resolution")
        .arg("resolution.json")
        .arg("--config")
        .arg("rustmanifest.toml")
        .arg("--scip")
        .arg(tree.path().join("absent.scip"))
        .arg(".")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("loading SCIP index"));
}

#[test]
fn allowed_writer_produces_no_invariant_finding() {
    let tree = tree_with_invariant();
    let output = rustmanifest()
        .current_dir(tree.path())
        .arg("check")
        .arg("--resolution")
        .arg("resolution.json")
        .arg("--config")
        .arg("rustmanifest.toml")
        .arg("--format")
        .arg("json")
        .arg(".")
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("`hack` writes"));
    assert!(!stdout.contains("`post` writes"));
}
