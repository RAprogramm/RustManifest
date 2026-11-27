// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Writes what a resolver settled about a workspace, as JSON.
//!
//! A binary of its own, and not a subcommand of the main one, so the
//! rust-analyzer libraries stay out of the tool a user runs on every save.
//! The main binary reads the file this one writes and never links the
//! resolver.

#![allow(
    clippy::multiple_crate_versions,
    reason = "the resolver's own dependency graph carries duplicates this crate cannot \
              deduplicate — embedded-io, foldhash, hashbrown, rustc-hash, syn, windows-sys \
              and winnow each arrive at two or three versions through rust-analyzer"
)]

use std::{
    io::Write as _,
    path::{Path, PathBuf},
    process::ExitCode
};

use masterror::AppError;
use rustmanifest_resolve::{ResolveError, resolve};

/// Exit status when the resolution was written.
const EXIT_CLEAN: u8 = 0;

/// Exit status when the run failed.
const EXIT_ERROR: u8 = 2;

fn main() -> ExitCode {
    let mut arguments = std::env::args().skip(1);
    let Some(root) = arguments.next().map(PathBuf::from) else {
        return fail("usage: rustmanifest-resolve <workspace> [out.json]");
    };
    let out = arguments
        .next()
        .map_or_else(|| PathBuf::from("resolution.json"), PathBuf::from);
    match write(&root, &out) {
        Ok(settled) => report(&format!(
            "{settled} calls settled, written to {}\n",
            out.display()
        )),
        Err(error) => fail(&error.to_string())
    }
}

/// Resolves `root` and writes the overlay to `out`, answering with how many
/// calls it settled.
fn write(root: &Path, out: &Path) -> Result<usize, AppError> {
    let resolution = resolve(root)?;
    let mut text = serde_json::to_string_pretty(&resolution).map_err(ResolveError::from)?;
    text.push('\n');
    std::fs::write(out, text).map_err(|source| ResolveError::File {
        path: out.to_path_buf(),
        source
    })?;
    Ok(resolution.calls.len())
}

/// Writes `message` where a reader expects the tool's answer.
fn report(message: &str) -> ExitCode {
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    if handle.write_all(message.as_bytes()).is_err() || handle.flush().is_err() {
        return ExitCode::from(EXIT_ERROR);
    }
    ExitCode::from(EXIT_CLEAN)
}

/// Reports a failure and answers with the failing status.
fn fail(message: &str) -> ExitCode {
    tracing::error!(error = message, "rustmanifest-resolve failed");
    ExitCode::from(EXIT_ERROR)
}
