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

use crate::manifests;

/// Aggregated expansion outcome across every discovered crate.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExpandSummary {
    /// Crate manifests discovered under the requested paths.
    pub crates_total:    u64,
    /// Crates whose expansion succeeded and parsed.
    pub crates_expanded: u64,
    /// Counters summed over every expanded crate.
    pub report:          ExpansionReport,
    /// Crate directory and expanded stream of each successfully
    /// expanded crate, for absorbing derived impls into the type graph
    /// or replacing the crate's sources in expanded analysis.
    pub streams:         Vec<(PathBuf, String)>
}

/// Expands every crate found under `paths` and aggregates the reports.
#[must_use]
pub fn expand_paths(paths: &[PathBuf]) -> ExpandSummary {
    let mut summary = ExpandSummary::default();
    for dir in manifest_dirs(paths) {
        summary.crates_total += 1;
        if let Some((report, stream)) = expand_crate(&dir) {
            summary.crates_expanded += 1;
            summary.report.absorb(&report);
            summary.streams.push((dir, stream));
        }
    }
    summary
}

/// Collects directories containing a `Cargo.toml` with a `[package]`
/// section, walking `paths` recursively and skipping hidden and `target`
/// directories.
fn manifest_dirs(paths: &[PathBuf]) -> Vec<PathBuf> {
    manifests::manifest_files(paths)
        .into_iter()
        .filter(|manifest| {
            fs::read_to_string(manifest).is_ok_and(|text| manifests::has_package_section(&text))
        })
        .filter_map(|manifest| manifest.parent().map(Path::to_path_buf))
        .collect()
}

/// Expands one crate, returning the measured report and the expanded
/// stream itself.
///
/// Library crates expand via `--lib`; binary-only crates via their first
/// `src/bin` entry or `src/main.rs` binary named after the package.
/// Expansion tries `--all-features` first so feature-gated code joins
/// the expanded universe — the same one-cfg-universe lesson the SCIP
/// layer documents — and falls back to default features for crates
/// whose feature set does not build all at once.
fn expand_crate(dir: &Path) -> Option<(ExpansionReport, String)> {
    let target = expand_target(dir)?;
    let stream =
        expand_stream(dir, &target, true).or_else(|| expand_stream(dir, &target, false))?;
    expansion(&stream).map(|report| (report, stream))
}

/// Runs one expansion attempt, with or without `--all-features`.
fn expand_stream(dir: &Path, target: &[String], all_features: bool) -> Option<String> {
    let mut command = Command::new("cargo");
    command.arg("+nightly").arg("rustc").arg("--profile=check");
    if all_features {
        command.arg("--all-features");
    }
    let output = command
        .args(target)
        .arg("--")
        .arg("-Zunpretty=expanded")
        .current_dir(dir)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

/// Picks the `cargo rustc` target selector for the crate in `dir`.
fn expand_target(dir: &Path) -> Option<Vec<String>> {
    if dir.join("src/lib.rs").is_file() {
        return Some(vec!["--lib".to_owned()]);
    }
    if dir.join("src/main.rs").is_file() {
        let name = manifests::package_name(&fs::read_to_string(dir.join("Cargo.toml")).ok()?)?;
        return Some(vec!["--bin".to_owned(), name]);
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
    use super::{expand_paths, expand_target, manifest_dirs};

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
