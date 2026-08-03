// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Checkout resolution: decides which tree of a pinned repository is
//! actually analyzed. A live checkout sitting on the pinned revision is
//! read in place; a drifted one is re-materialized from `git archive`, and
//! only an unavailable revision falls back to the live tree.

use std::{
    path::{Path, PathBuf},
    process::Command
};

use tracing::warn;

use crate::corpus::RepoEntry;

/// Revision recorded when `git rev-parse HEAD` cannot be resolved.
pub const UNKNOWN_REV: &str = "unknown";

/// The tree chosen for analysis together with the revision its sources
/// correspond to.
pub(super) struct AnalyzedTree {
    /// Directory whose sources are discovered and analyzed.
    pub(super) path:         PathBuf,
    /// Revision the analyzed sources correspond to.
    pub(super) rev:          String,
    /// Extraction backing `path`, kept alive for as long as it is read.
    pub(super) materialized: Option<tempfile::TempDir>
}

/// Selects the tree to analyze for `entry`, given its live checkout at
/// `repo_path`. Drift is tolerated and logged as a warning.
pub(super) fn select(entry: &RepoEntry, repo_path: PathBuf) -> AnalyzedTree {
    let actual_rev = head_revision(&repo_path);
    if actual_rev == entry.rev {
        return AnalyzedTree {
            path:         repo_path,
            rev:          entry.rev.clone(),
            materialized: None
        };
    }
    if let Some(pinned_tree) = materialize_rev(&repo_path, &entry.rev) {
        warn!(
            repo = %entry.name,
            pinned = %entry.rev,
            actual = %actual_rev,
            "live checkout drifted; analyzing the pinned revision from git archive"
        );
        return AnalyzedTree {
            path:         pinned_tree.path().to_path_buf(),
            rev:          entry.rev.clone(),
            materialized: Some(pinned_tree)
        };
    }
    warn!(
        repo = %entry.name,
        pinned = %entry.rev,
        actual = %actual_rev,
        "pinned revision not materializable; analyzing the live tree, counts may not be comparable"
    );
    AnalyzedTree {
        path:         repo_path,
        rev:          actual_rev,
        materialized: None
    }
}

/// Resolves the checked-out revision of `path`, falling back to
/// [`UNKNOWN_REV`] when the directory is not a resolvable git checkout.
fn head_revision(path: &Path) -> String {
    Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map_or_else(
            || UNKNOWN_REV.to_owned(),
            |output| String::from_utf8_lossy(&output.stdout).trim().to_owned()
        )
}

/// Extracts `rev` from the repository at `path` into a temporary directory
/// via `git archive`, returning `None` when the revision is unavailable.
fn materialize_rev(path: &Path, rev: &str) -> Option<tempfile::TempDir> {
    let dir = tempfile::TempDir::new().ok()?;
    let archive = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["archive", "--format=tar", rev])
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    let mut child = Command::new("tar")
        .arg("-x")
        .arg("-C")
        .arg(dir.path())
        .stdin(std::process::Stdio::piped())
        .spawn()
        .ok()?;
    {
        use std::io::Write;

        let stdin = child.stdin.as_mut()?;
        stdin.write_all(&archive.stdout).ok()?;
    }
    child
        .wait()
        .ok()
        .filter(std::process::ExitStatus::success)?;
    Some(dir)
}
