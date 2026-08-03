// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Which analyzed files a crate root reaches.
//!
//! A file is reachable once at least one module living in it hangs off a
//! cargo-layout crate root. A file no layout root claims is promoted to a
//! standalone crate root of its own and counted apart, so the two buckets
//! are disjoint by construction and a file is never counted twice.

use std::collections::BTreeSet;

use super::report::count;
use crate::semantic::module_graph::ModuleNode;

/// Split of the analyzed files into layout-reachable and standalone.
pub(super) struct Reach {
    /// Distinct files that became at least one module reachable from a
    /// cargo-layout crate root.
    pub(super) reachable:  u64,
    /// Distinct files claimed by no layout root and promoted to standalone
    /// crate roots of their own.
    pub(super) standalone: u64
}

/// Splits the files behind `modules` into layout-reachable and standalone.
pub(super) fn reach(modules: &[ModuleNode]) -> Reach {
    let reachable: BTreeSet<usize> = modules
        .iter()
        .filter(|module| module.layout)
        .map(|module| module.source)
        .collect();
    let standalone: BTreeSet<usize> = modules
        .iter()
        .filter(|module| !module.layout && !reachable.contains(&module.source))
        .map(|module| module.source)
        .collect();
    Reach {
        reachable:  count(reachable.len()),
        standalone: count(standalone.len())
    }
}
