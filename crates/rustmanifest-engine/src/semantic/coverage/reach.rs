// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Which analyzed files a build reaches.
//!
//! A file is reachable once the compiler's module tree places a module in
//! it. A file the tree never mentions is one no build compiles — a fixture
//! beside a rule, a loose script — and it is counted apart. The two
//! buckets are disjoint by construction and a file is never counted twice.

use std::collections::BTreeSet;

use super::report::count;
use crate::semantic::module_graph::ModuleNode;

/// Split of the analyzed files into layout-reachable and standalone.
pub(super) struct Reach {
    /// Distinct files the compiler's tree places a module in.
    pub(super) reachable:  u64,
    /// Distinct analyzed files the tree never mentions.
    pub(super) standalone: u64
}

/// Splits `files` analyzed files into those the tree reaches and those it
/// never mentions.
pub(super) fn reach(modules: &[ModuleNode], files: usize) -> Reach {
    let reachable: BTreeSet<usize> = modules.iter().map(|module| module.source).collect();
    Reach {
        standalone: count(files.saturating_sub(reachable.len())),
        reachable:  count(reachable.len())
    }
}
