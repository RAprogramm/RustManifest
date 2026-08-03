// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Crate names declared around the analyzed paths.

use std::{collections::BTreeSet, path::PathBuf};

use crate::manifests;

/// Collects the crate names known under `paths` — declared dependencies
/// plus the workspace's own package and library-target names — in their
/// in-code spelling, so the coverage report can attribute non-workspace
/// `use` targets to a known origin. Tests, benches, and examples import
/// their own workspace crates by name, which is why provided names count.
pub(super) fn declared_dependencies(paths: &[PathBuf]) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for manifest in manifests::manifest_files(paths) {
        if let Ok(text) = std::fs::read_to_string(&manifest) {
            names.extend(manifests::dependency_names(&text));
            names.extend(manifests::provided_crate_names(&text));
        }
    }
    names
}
