// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Cargo target-layout knowledge: which files are crate roots and how
//! directory context resolves them. Extending recognized layouts means
//! editing only this file.

use std::path::Path;

/// Returns true when `path` names a crate root file of cargo's target
/// layout: `src/lib.rs`, `src/main.rs`, `build.rs`, a file under
/// `src/bin/`, or a flat file / `*/main.rs` at the top level of `tests/`,
/// `benches/`, or `examples/`.
pub(super) fn is_crate_root(path: &Path) -> bool {
    const TARGET_DIRS: [&str; 3] = ["tests", "benches", "examples"];
    if path.ends_with("src/lib.rs") || path.ends_with("src/main.rs") {
        return true;
    }
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    if name == "build.rs" {
        return true;
    }
    let parent = parent_dir_name(path);
    if parent == Some("bin") && dir_name_at(path, 2) == Some("src") {
        return true;
    }
    if name == "main.rs"
        && dir_name_at(path, 2) == Some("bin")
        && dir_name_at(path, 3) == Some("src")
    {
        return true;
    }
    if parent.is_some_and(|dir| TARGET_DIRS.contains(&dir)) {
        return true;
    }
    name == "main.rs" && dir_name_at(path, 2).is_some_and(|dir| TARGET_DIRS.contains(&dir))
}

/// Returns the name of the directory directly containing `path`.
fn parent_dir_name(path: &Path) -> Option<&str> {
    dir_name_at(path, 1)
}

/// Returns the name of the directory `levels` components above the file.
fn dir_name_at(path: &Path, levels: usize) -> Option<&str> {
    let mut current = path;
    for _ in 0..levels {
        current = current.parent()?;
    }
    current.file_name()?.to_str()
}
