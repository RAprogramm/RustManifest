// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The judgement bed: a disposable copy of one checkout the landed
//! texts are built in, so the real tree is never touched.

use std::{
    path::{Path, PathBuf},
    process::Command
};

use super::candidate::Candidate;
use crate::error::EvalError;

/// One repository's bed: the copied tree and its temporary home.
pub(super) struct Bed {
    /// Keeps the temporary home alive for the bed's lifetime.
    _home: tempfile::TempDir,
    root:  PathBuf
}

impl Bed {
    /// Copies the checkout at `origin` into a fresh bed, skipping the
    /// build directory and the version-control store.
    ///
    /// # Errors
    ///
    /// Returns an [`EvalError`] when the copy cannot be built.
    pub(super) fn of(origin: &Path, name: &str) -> Result<Self, EvalError> {
        let home = tempfile::tempdir().map_err(|source| EvalError::Bed {
            name: name.to_owned(),
            source
        })?;
        let root = home.path().join(name);
        copied(origin, &root).map_err(|source| EvalError::Bed {
            name: name.to_owned(),
            source
        })?;
        Ok(Self {
            _home: home,
            root
        })
    }

    /// The copied tree's root.
    pub(super) fn root(&self) -> &Path {
        self.root.as_path()
    }

    /// Lands one candidate, judges it with the real build, restores the
    /// tree, and answers the compiler's first error line where it
    /// refused — nothing where it accepted.
    ///
    /// # Errors
    ///
    /// Returns an [`EvalError`] when landing, building, or restoring
    /// fails operationally.
    pub(super) fn judged(&self, candidate: &Candidate) -> Result<Option<String>, EvalError> {
        let named = |source: std::io::Error| EvalError::Bed {
            name: self.root.display().to_string(),
            source
        };
        let kept = std::fs::read_to_string(&candidate.file).map_err(named)?;
        std::fs::write(&candidate.file, &candidate.installed).map_err(named)?;
        if let Some(planted) = &candidate.planted {
            if let Some(parent) = planted.file.parent() {
                std::fs::create_dir_all(parent).map_err(named)?;
            }
            std::fs::write(&planted.file, &planted.text).map_err(named)?;
        }
        let built = Command::new("cargo")
            .arg("check")
            .arg("--workspace")
            .arg("--offline")
            .arg("--quiet")
            .current_dir(&self.root)
            .output()
            .map_err(named)?;
        std::fs::write(&candidate.file, kept).map_err(named)?;
        if let Some(planted) = &candidate.planted {
            std::fs::remove_file(&planted.file).map_err(named)?;
        }
        if built.status.success() {
            return Ok(None);
        }
        let complaint = String::from_utf8_lossy(&built.stderr);
        Ok(Some(
            complaint
                .lines()
                .find(|line| line.starts_with("error"))
                .unwrap_or("the build failed without an error line")
                .to_owned()
        ))
    }
}

/// Copies a tree, skipping `target` and `.git` at every depth.
fn copied(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == "target" || name == ".git" {
            continue;
        }
        let source = entry.path();
        let landed = to.join(&name);
        if entry.file_type()?.is_dir() {
            copied(&source, &landed)?;
        } else {
            std::fs::copy(&source, &landed)?;
        }
    }
    Ok(())
}
