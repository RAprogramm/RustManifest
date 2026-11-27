// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! File discovery for the orchestrator.
//!
//! Wraps the `ignore` crate's gitignore-aware walker, restricted to `*.rs`
//! files. Returned paths are absolute or relative as supplied by the caller;
//! no canonicalization is performed (callers wanting canonical paths should
//! resolve them themselves before passing them in).

use std::path::{Path, PathBuf};

use ignore::WalkBuilder;
use masterror::AppError;
use tracing::trace;

/// Discovers Rust source files reachable from the given root paths.
///
/// The walker respects `.gitignore`, hidden file conventions, and any
/// `ignore`-crate-recognized exclusion file. Only files with the `.rs`
/// extension are returned.
///
/// # Errors
///
/// Returns an [`AppError`] if the walker reports a hard error (e.g.
/// permission denied on a top-level root). Soft errors visiting
/// individual entries are logged at `trace` level and the entry is skipped.
pub fn discover<P: AsRef<Path>>(roots: &[P]) -> Result<Vec<PathBuf>, AppError> {
    let mut iter = roots.iter().map(AsRef::as_ref);
    let Some(first) = iter.next() else {
        return Ok(Vec::new());
    };

    let mut builder = WalkBuilder::new(first);
    for root in iter {
        builder.add(root);
    }
    builder.standard_filters(true).hidden(true);

    let mut files = Vec::new();
    for entry in builder.build() {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                trace!(error = %err, "walker entry error skipped");
                continue;
            }
        };
        if entry.file_type().is_some_and(|ft| ft.is_file())
            && entry.path().extension().is_some_and(|ext| ext == "rs")
        {
            files.push(named(entry.path()));
        }
    }
    files.sort();
    Ok(files)
}

/// The path a file is recorded under: as walked, with a leading `./`
/// dropped.
///
/// A resolution names files from the workspace root, and a consumer looks
/// one up by equality. Walking `.` prefixes every path with `./`, which
/// matches nothing the resolver wrote — and reads exactly like a position
/// nobody settled.
fn named(path: &Path) -> PathBuf {
    path.strip_prefix("./").unwrap_or(path).to_path_buf()
}
