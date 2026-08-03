// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Where the active toolchain keeps the library's own sources.
//!
//! Every derivation reads real files, so it needs the tree the installed
//! toolchain ships. The compiler itself is asked for the sysroot; an
//! answer that does not lead to an installed source component yields
//! nothing, and the caller derives nothing rather than guessing a path.

use std::path::{Path, PathBuf};

/// Path of the standard library's source tree for the active toolchain,
/// when the component is installed.
#[must_use]
pub fn std_source_root() -> Option<PathBuf> {
    let output = std::process::Command::new("rustc")
        .arg("--print")
        .arg("sysroot")
        .output()
        .ok()?;
    let sysroot = String::from_utf8(output.stdout).ok()?;
    let root = Path::new(sysroot.trim()).join("lib/rustlib/src/rust/library");
    root.is_dir().then_some(root)
}
