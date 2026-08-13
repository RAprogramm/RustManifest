// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The context the reachability fold reads beyond the syntax it walks.

use std::collections::BTreeMap;


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
    /// Byte ranges whose settled type is the never type, so a call
    /// written there does not come back.
    pub(super) leaving: &'a std::collections::BTreeSet<(u32, u32)>
}
