// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Workspace typing tables shared by every function's fact extraction:
//! what each field of each struct holds, what each function returns, what
//! each named string constant stands for, and which crate every source
//! belongs to.
//!
//! The tables that have to walk the whole workspace to be built sit in
//! submodules beside this file; crate identity reads a path and stays
//! here.

mod constant;

use super::scoped::Scoped;
use crate::source::Source;

/// Workspace typing tables shared by every function's fact extraction.
pub(super) struct TypeTables {
    /// Names of string constants to the literal each is written as.
    pub(super) constants: Scoped<String>,
    /// Crate identity of every source, for crate-proximity resolution.
    pub(super) crates:    Vec<String>
}

impl TypeTables {
    /// Reads every typing table off `sources` in one pass each.
    pub(super) fn of(sources: &[Source]) -> Self {
        Self {
            constants: constant::constant_texts(sources),
            crates:    crate_ids(sources)
        }
    }
}

/// Crate identity of every source, from the path component preceding
/// its `src` directory.
fn crate_ids(sources: &[Source]) -> Vec<String> {
    sources
        .iter()
        .map(|source| {
            let path = source.path().to_string_lossy().replace('\\', "/");
            let components: Vec<&str> = path.split('/').filter(|c| !c.is_empty()).collect();
            components
                .iter()
                .position(|component| *component == "src")
                .and_then(|position| position.checked_sub(1))
                .and_then(|index| components.get(index))
                .map_or_default(|name| (*name).to_owned())
        })
        .collect()
}
