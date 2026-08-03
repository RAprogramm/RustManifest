// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The context the reachability fold reads beyond the syntax it walks.

use std::collections::BTreeMap;

use crate::semantic::behavior::{Returned, Scoped};

/// The context the reachability fold reads beyond the syntax it walks.
///
/// It carries the enclosing file's `use` table, and that is the reason
/// the fold is a set of methods rather than free functions: a name is
/// only a path once the imports have been applied to it, and a
/// recognizer that skips that step disagrees with every other
/// recognizer in this module about the same call.
pub(super) struct FlowScope<'a> {
    /// Imported names of the enclosing file to their full `use` paths.
    pub(super) imports: &'a BTreeMap<String, String>,
    /// Workspace functions to their return types, so a call to a
    /// function whose signature declares the never type is read as the
    /// divergence it is.
    pub(super) returns: &'a Scoped<Returned>,
    /// Source index of the function under analysis, for scoped lookup.
    pub(super) src:     usize,
    /// Crate identity of every source, for scoped lookup.
    pub(super) crates:  &'a [String]
}
