// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What every command that reads a program takes: the paths to read, and
//! what a resolver settled about them.
//!
//! The two travel together because neither answers alone — a resolution is
//! about the sources it was taken over, and reading it against other paths
//! would state of one program what holds of another. Four commands
//! declared the pair separately before.

use std::path::PathBuf;

use clap::Args;

/// The program under analysis, as a command names it.
#[derive(Debug, Args)]
pub struct Analyzed {
    /// Files or directories to analyze. Defaults to the current directory.
    #[arg(default_value = ".")]
    pub paths: Vec<PathBuf>,

    /// Resolution overlay written by `rustmanifest-resolve`, so a call into
    /// a library states the type it hands back instead of a refusal.
    #[arg(long)]
    pub resolution: Option<PathBuf>
}

impl Analyzed {
    /// Reads the resolution overlay this names, where one was named.
    ///
    /// A named file that cannot be read or parsed is an error and never a
    /// silent fallback to no resolution: a caller who asked for resolved
    /// answers must not be handed unresolved ones without being told.
    ///
    /// # Errors
    ///
    /// Returns the failure of reading or parsing the named file.
    pub(crate) fn resolution(&self) -> anyhow::Result<Option<rustmanifest_schema::Resolution>> {
        crate::resolution(
            self.resolution.as_deref(),
            self.paths
                .first()
                .map_or_else(|| std::path::Path::new("."), PathBuf::as_path)
        )
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use super::*;

    #[test]
    fn naming_no_overlay_reads_none() {
        let analyzed = Analyzed {
            paths:      vec![PathBuf::from(".")],
            resolution: None
        };
        assert!(analyzed.resolution().unwrap().is_none());
    }

    #[test]
    fn naming_an_overlay_that_is_not_there_is_an_error() {
        let analyzed = Analyzed {
            paths:      vec![PathBuf::from(".")],
            resolution: Some(PathBuf::from("no/such/overlay.json"))
        };
        assert!(analyzed.resolution().is_err());
    }
}
