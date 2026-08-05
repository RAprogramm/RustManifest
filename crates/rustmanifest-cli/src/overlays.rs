// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The two ways a command can be given more than the source text alone.
//!
//! Macro expansion adds sources the author never wrote; a name index adds
//! call edges the syntactic resolver could not settle. Both widen what a
//! run can see without changing what it claims, and both are asked for the
//! same way, so they are named once here rather than in every command that
//! accepts them.

use std::path::PathBuf;

use clap::Args;

/// What a command may be handed beside the sources.
#[derive(Debug, Args)]
pub struct Overlays {
    /// Expand every discovered crate via `cargo +nightly rustc -- \
    /// -Zunpretty=expanded` (all features, falling back to default); a
    /// successfully expanded crate's stream replaces its `src` files, so
    /// macro-generated functions become facts. The expanded universe is
    /// the compiled library: `#[cfg(test)]` modules are absent from it.
    /// Crates that fail to expand keep their plain sources.
    #[arg(long)]
    pub expand: bool,

    /// SCIP index file (`rust-analyzer scip .`) overlaying name-resolved
    /// call edges onto the graph.
    #[arg(long, value_name = "INDEX")]
    pub scip: Option<PathBuf>
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
    fn asking_for_neither_is_the_plain_reading() {
        let overlays = Overlays {
            expand: false,
            scip:   None
        };
        assert!(!overlays.expand);
        assert!(overlays.scip.is_none());
    }

    #[test]
    fn each_is_asked_for_on_its_own() {
        let expanded = Overlays {
            expand: true,
            scip:   None
        };
        let indexed = Overlays {
            expand: false,
            scip:   Some(PathBuf::from("index.scip"))
        };
        assert!(expanded.expand && expanded.scip.is_none());
        assert!(!indexed.expand && indexed.scip.is_some());
    }
}
