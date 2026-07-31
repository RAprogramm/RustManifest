// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Macro-expansion runner backing `coverage --expand`.
//!
//! Discovers crate manifests under the requested paths, expands each crate
//! through `cargo +nightly rustc -- -Zunpretty=expanded`, and feeds the
//! expanded stream to the engine's [`expansion`] measurement. Crates whose
//! expansion fails — no nightly toolchain, broken build, unparsable
//! output — are counted as skipped, never guessed at.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command
};

use rustmanifest_engine::{ExpansionReport, expansion};

/// Aggregated expansion outcome across every discovered crate.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExpandSummary {
    /// Crate manifests discovered under the requested paths.
    pub crates_total:    u64,
    /// Crates whose expansion succeeded and parsed.
    pub crates_expanded: u64,
    /// Counters summed over every expanded crate.
    pub report:          ExpansionReport
}

/// Expands every crate found under `paths` and aggregates the reports.
#[must_use]
pub fn expand_paths(paths: &[PathBuf]) -> ExpandSummary {
    let mut summary = ExpandSummary::default();
    for dir in manifest_dirs(paths) {
        summary.crates_total += 1;
        if let Some(report) = expand_crate(&dir) {
            summary.crates_expanded += 1;
            summary.report.absorb(&report);
        }
    }
    summary
}

/// Collects directories containing a `Cargo.toml` with a `[package]`
/// section, walking `paths` recursively and skipping hidden and `target`
/// directories.
fn manifest_dirs(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for path in paths {
        walk_manifests(path, &mut dirs);
    }
    dirs.sort();
    dirs.dedup();
    dirs
}

/// Recursive step of [`manifest_dirs`].
fn walk_manifests(dir: &Path, dirs: &mut Vec<PathBuf>) {
    let manifest = dir.join("Cargo.toml");
    if manifest.is_file()
        && let Ok(text) = fs::read_to_string(&manifest)
        && text.lines().any(|line| line.trim() == "[package]")
    {
        dirs.push(dir.to_path_buf());
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() || skipped_dir_name(&path) {
            continue;
        }
        walk_manifests(&path, dirs);
    }
}

/// Returns whether a directory is hidden or a build-output directory.
fn skipped_dir_name(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_none_or(|name| name.starts_with('.') || name == "target")
}

/// Expands one crate and measures the expanded stream.
///
/// Library crates expand via `--lib`; binary-only crates via their first
/// `src/bin` entry or `src/main.rs` binary named after the package.
fn expand_crate(dir: &Path) -> Option<ExpansionReport> {
    let target = expand_target(dir)?;
    let output = Command::new("cargo")
        .arg("+nightly")
        .arg("rustc")
        .arg("--profile=check")
        .args(&target)
        .arg("--")
        .arg("-Zunpretty=expanded")
        .current_dir(dir)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    expansion(&String::from_utf8(output.stdout).ok()?)
}

/// Picks the `cargo rustc` target selector for the crate in `dir`.
fn expand_target(dir: &Path) -> Option<Vec<String>> {
    if dir.join("src/lib.rs").is_file() {
        return Some(vec!["--lib".to_owned()]);
    }
    if dir.join("src/main.rs").is_file() {
        let name = package_name(&fs::read_to_string(dir.join("Cargo.toml")).ok()?)?;
        return Some(vec!["--bin".to_owned(), name]);
    }
    None
}

/// Extracts the package name from a manifest's `[package]` section.
fn package_name(manifest: &str) -> Option<String> {
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

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use super::{expand_paths, expand_target, manifest_dirs, package_name};

    #[test]
    fn package_name_is_read_from_the_package_section() {
        let manifest = "[workspace]\nmembers = [\"a\"]\n[package]\nname = \
                        \"probe\"\nversion = \"0.1.0\"\n[dependencies]\nname = \"decoy\"\n";
        assert_eq!(package_name(manifest).unwrap(), "probe");
    }

    #[test]
    fn manifest_without_package_section_has_no_name() {
        assert!(package_name("[workspace]\nmembers = []\n").is_none());
    }

    #[test]
    fn virtual_manifests_and_hidden_dirs_are_skipped() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("Cargo.toml"), "[workspace]\n").unwrap();
        let member = root.path().join("member");
        std::fs::create_dir_all(member.join("src")).unwrap();
        std::fs::write(member.join("Cargo.toml"), "[package]\nname = \"member\"\n").unwrap();
        let hidden = root.path().join(".hidden/inner");
        std::fs::create_dir_all(&hidden).unwrap();
        std::fs::write(hidden.join("Cargo.toml"), "[package]\nname = \"h\"\n").unwrap();
        let dirs = manifest_dirs(&[root.path().to_path_buf()]);
        assert_eq!(dirs, vec![member]);
    }

    #[test]
    fn lib_crates_expand_via_lib_target() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("src")).unwrap();
        std::fs::write(root.path().join("src/lib.rs"), "").unwrap();
        assert_eq!(expand_target(root.path()).unwrap(), vec!["--lib"]);
    }

    #[test]
    fn bin_crates_expand_via_the_package_name() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("src")).unwrap();
        std::fs::write(root.path().join("src/main.rs"), "fn main() {}").unwrap();
        std::fs::write(
            root.path().join("Cargo.toml"),
            "[package]\nname = \"tool\"\n"
        )
        .unwrap();
        assert_eq!(expand_target(root.path()).unwrap(), vec!["--bin", "tool"]);
    }

    #[test]
    fn empty_tree_yields_an_empty_summary() {
        let root = tempfile::tempdir().unwrap();
        let summary = expand_paths(&[root.path().to_path_buf()]);
        assert_eq!(summary.crates_total, 0);
        assert_eq!(summary.crates_expanded, 0);
    }
}
