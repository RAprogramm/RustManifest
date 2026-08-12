// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Resolving a real cargo workspace end to end.
//!
//! The unit tests cover the shape of the overlay; only this one proves the
//! part that matters — that a call into the standard library is settled at
//! all. Without the standard-library sources attached it is not, and the
//! overlay comes back nearly empty while every unit test still passes.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::multiple_crate_versions,
    reason = "test code is allowed to fail loudly, and the resolver's dependency graph carries \
              duplicate versions this crate cannot deduplicate"
)]

use std::{fs, path::Path};

use rustmanifest_resolve::resolve;

/// Writes a minimal cargo project whose body calls into the standard
/// library and into itself.
fn fixture(root: &Path) {
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        b"[package]\nname = \"fixture\"\nversion = \"0.0.0\"\nedition = \"2024\"\n"
    )
    .unwrap();
    fs::write(
        root.join("src/main.rs"),
        b"struct Store { count: u64 }\n\
          impl Store {\n\
          \x20   fn new() -> Self { Self { count: 1 } }\n\
          \x20   fn find(&self) -> Option<u64> { Some(self.count) }\n\
          }\n\
          fn main() {\n\
          \x20   let store = Store::new();\n\
          \x20   let listed = store.find().is_some();\n\
          \x20   println!(\"{listed}\");\n\
          }\n"
    )
    .unwrap();
}

#[test]
fn a_call_into_the_standard_library_is_settled_with_its_type() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    let resolution = resolve(dir.path()).expect("the fixture workspace loads");

    let settled: Vec<(&str, &str)> = resolution
        .calls
        .iter()
        .map(|call| (call.callee.as_str(), call.returns.as_str()))
        .collect();
    assert!(
        settled.contains(&("is_some", "bool")),
        "the standard-library call was not settled: {settled:?}"
    );
    assert!(
        settled.contains(&("find", "Option<u64>")),
        "the workspace call was not settled: {settled:?}"
    );
}

#[test]
fn a_settled_call_carries_the_type_of_its_receiver() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    let resolution = resolve(dir.path()).expect("the fixture workspace loads");

    let receiver = resolution
        .calls
        .iter()
        .find(|call| call.callee == "is_some")
        .and_then(|call| call.receiver.as_deref());
    assert_eq!(receiver, Some("Option<u64>"));
}

#[test]
fn every_expression_carries_its_type_and_not_only_the_calls() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    let resolution = resolve(dir.path()).expect("the fixture workspace loads");

    let rendered: Vec<&str> = resolution
        .types
        .iter()
        .map(|typed| typed.rendered.as_str())
        .collect();
    assert!(
        rendered.contains(&"Store"),
        "the binding read was not typed: {rendered:?}"
    );
    assert!(
        rendered.contains(&"bool"),
        "the outermost expression was not typed: {rendered:?}"
    );
    assert!(
        rendered.contains(&"Option<u64>"),
        "the intermediate call was not typed: {rendered:?}"
    );
}

/// Every consumer walks the tree itself and names a file the way it was
/// asked to walk it. A path stated from the machine's root matches none of
/// those names, and the overlay is read by equality — so the mismatch does
/// not fail, it answers nothing, everywhere, forever.
#[test]
fn a_settled_position_names_its_file_the_way_the_workspace_does() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    let resolution = resolve(dir.path()).expect("the fixture workspace loads");

    let named: Vec<&Path> = resolution
        .calls
        .iter()
        .map(|call| call.at.file.as_path())
        .chain(resolution.types.iter().map(|typed| typed.at.file.as_path()))
        .chain(resolution.paths.iter().map(|path| path.at.file.as_path()))
        .collect();
    assert!(!named.is_empty());
    assert!(
        named.iter().all(|path| path.is_relative()),
        "a settled position states an absolute path: {named:?}"
    );
    assert!(
        named.iter().all(|path| path.starts_with("src")),
        "a settled position is not stated from the workspace root: {named:?}"
    );
}

#[test]
fn a_written_name_carries_the_item_it_reaches_and_the_crate_declaring_it() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    let resolution = resolve(dir.path()).expect("the fixture workspace loads");

    let store = resolution
        .paths
        .iter()
        .find(|path| path.names.ends_with("Store"))
        .expect("the written type name reaches an item");
    assert_eq!(store.kind, "struct");
    assert_eq!(store.declaring, "fixture");
    assert_eq!(store.names, "fixture::Store");
}

/// The compiler's own module tree is recorded, so nothing above has to
/// guess which file holds which module from how the files are named.
#[test]
fn the_module_tree_is_the_compilers_and_not_a_reading_of_file_names() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    let resolution = resolve(dir.path()).expect("the fixture workspace loads");

    let root = resolution
        .modules
        .iter()
        .find(|module| module.crate_root && module.names == "fixture")
        .expect("the crate root is recorded");
    assert_eq!(root.file, Path::new("src/main.rs"));
    assert!(root.parent.is_none());
    assert!(
        resolution
            .modules
            .iter()
            .all(|module| module.file.is_relative()),
        "a module names its file from the machine root: {:?}",
        resolution.modules
    );
}

/// A call names the function it reaches by path, and every declared
/// function is named the same way — so a reader binds a call to a body by
/// identity instead of by a shared spelling.
#[test]
fn a_call_and_the_body_it_reaches_are_named_the_same_way() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    let resolution = resolve(dir.path()).expect("the fixture workspace loads");

    let reached = resolution
        .calls
        .iter()
        .find(|call| call.callee == "find")
        .and_then(|call| call.reaches.as_deref())
        .expect("the workspace call names what it reaches");
    assert_eq!(reached, "fixture::Store::find");
    assert!(
        resolution
            .functions
            .iter()
            .any(|declared| declared.names == reached),
        "no declared function carries that name: {:?}",
        resolution.functions
    );
}

#[test]
fn resolving_the_same_workspace_twice_states_the_same_thing() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    let first = resolve(dir.path()).expect("the fixture workspace loads");
    let second = resolve(dir.path()).expect("the fixture workspace loads");
    assert_eq!(first, second);
}

#[test]
fn a_directory_that_is_no_workspace_is_an_error_and_not_an_empty_answer() {
    let dir = tempfile::tempdir().unwrap();
    assert!(resolve(dir.path()).is_err());
}
