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

/// Writes the module tree a resolver would state for these files and
/// answers the path of the overlay.
fn overlay(
    dir: &std::path::Path,
    rows: &[(&str, &str)],
    reached: &[(&str, &str, u32, u32)]
) -> std::path::PathBuf {
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
    let paths: Vec<String> = reached
        .iter()
        .map(|(file, names, start, end)| {
            format!(
                "{{\"at\": {{\"file\": \"{file}\", \"range\": {{\"start\": {start}, \"end\": \
                 {end}}}}}, \"names\": \"{names}\", \"kind\": \"struct\", \"declaring\": \
                 \"demo\"}}"
            )
        })
        .collect();
    let text = format!(
        "{{\"resolver\": \"test\", \"calls\": [], \"modules\": [{}], \"paths\": [{}]}}",
        modules.join(", "),
        paths.join(", ")
    );
    let path = dir.join("resolution.json");
    std::fs::write(&path, text).unwrap();
    path
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

/// Writes beside `tree` what a compiler settles about its sources, and
/// answers with the path the graph reads it from.
///
/// The type graph names a field by the type the compiler settled for
/// it, so a run over a bare directory draws no edge at all.
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
fn modules_mermaid_contains_header_nodes_and_edge() {
    let tree = make_tree(&[
        ("src/lib.rs", "mod util;\n"),
        ("src/util.rs", "use crate::Item;\n")
    ]);
    let settled = overlay(
        tree.path(),
        &[("demo", "src/lib.rs"), ("demo::util", "src/util.rs")],
        &[("src/util.rs", "demo::Item", 11, 15)]
    );
    rustmanifest()
        .current_dir(tree.path())
        .arg("graph")
        .arg("--resolution")
        .arg(&settled)
        .arg(".")
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
    let settled = overlay(
        tree.path(),
        &[
            ("demo", "src/lib.rs"),
            ("demo::a", "src/lib.rs"),
            ("demo::b", "src/lib.rs")
        ],
        &[
            ("src/lib.rs", "demo::b::Item", 22, 26),
            ("src/lib.rs", "demo::a::Other", 52, 57)
        ]
    );
    rustmanifest()
        .current_dir(tree.path())
        .arg("graph")
        .arg("--resolution")
        .arg(&settled)
        .arg(".")
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
    let settled = overlay(
        tree.path(),
        &[
            ("demo", "src/lib.rs"),
            ("demo::a", "src/lib.rs"),
            ("demo::b", "src/lib.rs")
        ],
        &[
            ("src/lib.rs", "demo::b::Item", 22, 26),
            ("src/lib.rs", "demo::a::Other", 52, 57)
        ]
    );
    rustmanifest()
        .current_dir(tree.path())
        .arg("graph")
        .arg("--resolution")
        .arg(&settled)
        .arg(".")
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
            "pub struct User { pub identity: crate::identity::Identity }\n"
        )
    ]);
    rustmanifest()
        .arg("graph")
        .arg(tree.path())
        .arg("--resolution")
        .arg(settled_overlay(&tree))
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
            "pub struct User { pub identity: crate::identity::Identity }\n"
        )
    ]);
    rustmanifest()
        .arg("graph")
        .arg(tree.path())
        .arg("--resolution")
        .arg(settled_overlay(&tree))
        .arg("--kind")
        .arg("types")
        .assert()
        .success()
        .stdout(predicate::str::contains("graph LR"))
        .stdout(predicate::str::contains("User -->|identity| Identity"));
}

#[test]
fn types_mermaid_shows_interaction_edges_dashed() {
    let tree = make_tree(&[
        (
            "src/board.rs",
            "pub struct Board;\nimpl Board { pub fn reset() {} }\n"
        ),
        (
            "src/game.rs",
            "pub struct Game;\nimpl Game { pub fn restart(&self) { crate::board::Board::reset(); } }\n"
        )
    ]);
    rustmanifest()
        .arg("graph")
        .arg(tree.path())
        .arg("--resolution")
        .arg(settled_overlay(&tree))
        .arg("--kind")
        .arg("types")
        .assert()
        .success()
        .stdout(predicate::str::contains("Game -.->|calls| Board"));
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
