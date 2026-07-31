// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Cargo manifest discovery and lightweight parsing shared by the
//! `coverage` and expansion machinery.
//!
//! The parsing here is deliberately shallow: it walks TOML section headers
//! and key names line by line, which is enough to list declared dependency
//! names and read a package name without pulling a TOML parser into the
//! CLI. Dependency names are normalized to their in-code form (hyphens
//! become underscores) because that is how `use` paths spell them.

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf}
};

/// Collects every `Cargo.toml` under `paths`, walking recursively and
/// skipping hidden and `target` directories.
#[must_use]
pub fn manifest_files(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for path in paths {
        walk(path, &mut files);
    }
    files.sort();
    files.dedup();
    files
}

/// Recursive step of [`manifest_files`].
fn walk(dir: &Path, files: &mut Vec<PathBuf>) {
    let manifest = dir.join("Cargo.toml");
    if manifest.is_file() {
        files.push(manifest);
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && !skipped_dir(&path) {
            walk(&path, files);
        }
    }
}

/// Returns whether a directory is hidden or a build-output directory.
fn skipped_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_none_or(|name| name.starts_with('.') || name == "target")
}

/// Returns whether the manifest declares a `[package]` section, i.e. is a
/// real crate rather than a virtual workspace root.
#[must_use]
pub fn has_package_section(manifest: &str) -> bool {
    manifest.lines().any(|line| line.trim() == "[package]")
}

/// Extracts the package name from a manifest's `[package]` section.
#[must_use]
pub fn package_name(manifest: &str) -> Option<String> {
    let mut in_package = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_package = trimmed == "[package]";
            continue;
        }
        if in_package
            && let Some(value) = trimmed.strip_prefix("name")
            && let Some(value) = value.trim_start().strip_prefix('=')
        {
            return Some(value.trim().trim_matches('"').to_owned());
        }
    }
    None
}

/// Lists dependency names declared anywhere in the manifest — regular,
/// dev, build, workspace, and target-specific dependency sections — in
/// their in-code spelling (hyphens replaced with underscores).
#[must_use]
pub fn dependency_names(manifest: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut in_dependencies = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if let Some(inner) = section_header(trimmed) {
            in_dependencies = false;
            let segments: Vec<&str> = inner.split('.').collect();
            match segments.iter().position(|segment| {
                *segment == "dependencies"
                    || *segment == "dev-dependencies"
                    || *segment == "build-dependencies"
            }) {
                Some(position) if position + 1 == segments.len() => in_dependencies = true,
                Some(position) => {
                    names.insert(normalized(segments[position + 1]));
                }
                None => {}
            }
            continue;
        }
        if in_dependencies
            && !trimmed.is_empty()
            && !trimmed.starts_with('#')
            && let Some((key, _)) = trimmed.split_once('=')
        {
            names.insert(normalized(key.trim().trim_matches('"')));
        }
    }
    names
}

/// Returns the inside of a `[section]` header line, or [`None`] for other
/// lines.
fn section_header(line: &str) -> Option<&str> {
    line.strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .map(str::trim)
}

/// Converts a dependency name to its in-code crate-path spelling.
fn normalized(name: &str) -> String {
    name.replace('-', "_")
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use super::{dependency_names, has_package_section, manifest_files, package_name};

    #[test]
    fn package_name_is_read_from_the_package_section() {
        let manifest = "[workspace]\nmembers = [\"a\"]\n[package]\nname = \
                        \"probe\"\nversion = \"0.1.0\"\n[dependencies]\nname = \"decoy\"\n";
        assert_eq!(package_name(manifest).unwrap(), "probe");
    }

    #[test]
    fn manifest_without_package_section_has_no_name() {
        assert!(package_name("[workspace]\nmembers = []\n").is_none());
        assert!(!has_package_section("[workspace]\nmembers = []\n"));
    }

    #[test]
    fn dependency_names_cover_every_dependency_section() {
        let manifest = "[package]\nname = \"probe\"\n[dependencies]\nserde = { version = \"1\" \
                        }\ntokio-util = \"0.7\"\n[dev-dependencies]\ntempfile = \
                        \"3\"\n[build-dependencies]\ncc = \"1\"\n[workspace.dependencies]\nsyn = \
                        \"2\"\n[target.'cfg(unix)'.dependencies]\nlibc = \
                        \"0.2\"\n[dependencies.uuid]\nversion = \"1\"\n";
        let names = dependency_names(manifest);
        let expected: Vec<&str> = vec![
            "cc",
            "libc",
            "serde",
            "syn",
            "tempfile",
            "tokio_util",
            "uuid",
        ];
        assert_eq!(
            names.iter().map(String::as_str).collect::<Vec<_>>(),
            expected
        );
    }

    #[test]
    fn package_keys_are_not_mistaken_for_dependencies() {
        let manifest = "[package]\nname = \"probe\"\nversion = \"0.1.0\"\n";
        assert!(dependency_names(manifest).is_empty());
    }

    #[test]
    fn nested_dependency_section_fields_are_not_dependencies() {
        let manifest = "[dependencies.serde]\nversion = \"1\"\nfeatures = [\"derive\"]\n";
        let names = dependency_names(manifest);
        assert_eq!(
            names.iter().map(String::as_str).collect::<Vec<_>>(),
            vec!["serde"]
        );
    }

    #[test]
    fn discovery_skips_hidden_and_target_directories() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("Cargo.toml"), "[workspace]\n").unwrap();
        let member = root.path().join("member");
        std::fs::create_dir_all(&member).unwrap();
        std::fs::write(member.join("Cargo.toml"), "[package]\nname = \"m\"\n").unwrap();
        for skipped in [".hidden", "target"] {
            let dir = root.path().join(skipped).join("inner");
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
        }
        let files = manifest_files(&[root.path().to_path_buf()]);
        assert_eq!(
            files,
            vec![root.path().join("Cargo.toml"), member.join("Cargo.toml")]
        );
    }
}
