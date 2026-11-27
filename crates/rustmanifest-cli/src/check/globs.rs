// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Include and exclude glob filtering of the discovered file list.

use std::path::PathBuf;

use anyhow::Context;
use globset::{Glob, GlobSet, GlobSetBuilder};
use rustmanifest_config::Effective;

/// Applies the configuration's include and exclude globs to the discovered
/// file list. Paths are matched as discovered; an empty include list keeps
/// every file.
pub(super) fn apply_globs(
    files: Vec<PathBuf>,
    effective: &Effective,
) -> anyhow::Result<Vec<PathBuf>> {
    let include = build_globset(effective.include()).context("compiling include globs")?;
    let exclude = build_globset(effective.exclude()).context("compiling exclude globs")?;
    Ok(files
        .into_iter()
        .filter(|path| include.as_ref().is_none_or(|set| set.is_match(path)))
        .filter(|path| exclude.as_ref().is_none_or(|set| !set.is_match(path)))
        .collect())
}

/// Compiles glob patterns into a single matcher; `None` when no patterns are
/// configured.
fn build_globset(patterns: &[String]) -> anyhow::Result<Option<GlobSet>> {
    if patterns.is_empty() {
        return Ok(None);
    }
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob =
            Glob::new(pattern).with_context(|| format!("invalid glob pattern {pattern:?}"))?;
        builder.add(glob);
    }
    Ok(Some(builder.build().context("building glob set")?))
}
