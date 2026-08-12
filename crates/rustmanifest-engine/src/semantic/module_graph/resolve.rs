// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Which module of the same crate an imported name reaches.
//!
//! Asked of the resolution, by the place the name is written at. What a
//! written path reaches is decided by the language's name resolution —
//! `crate`, `self`, `super`, a uniform path, an import that renames
//! another import — and the arithmetic that stood here rewrote those by
//! hand and then picked the deepest module whose path covered the result.
//!
//! Without a resolution nothing is reached: an import is a name, and which
//! item a name reaches is the question this cannot answer for itself.

use rustmanifest_schema::{Location, Resolution};

use super::{ModuleNode, UseTarget};
use crate::source::Source;

/// Resolves the name written at `use_target`, inside the module at `from`,
/// to a module of the same crate.
///
/// A name reaching an item answers with the module declaring that item, so
/// an import of a type resolves to the module it is declared in. A name
/// reaching outside the crate, or one no resolution settled, answers with
/// nothing.
pub(in crate::semantic) fn resolved_module(
    modules: &[ModuleNode],
    sources: &[Source],
    settled: Option<&Resolution>,
    from: usize,
    use_target: &UseTarget
) -> Option<usize> {
    let source = modules.get(from)?;
    let file = sources.get(source.source)?.path().to_path_buf();
    let reached = settled?.path_at(&Location {
        file,
        range: use_target.range
    })?;
    let inside = below_crate(&reached.names)?;
    modules
        .iter()
        .enumerate()
        .filter(|(_, module)| module.krate == source.krate)
        .filter(|(_, module)| covers(&module.path, &inside))
        .max_by_key(|(_, module)| module.path.len())
        .map(|(index, _)| index)
}

/// The path an item is reached by, with the crate name replaced by the
/// keyword the graph spells module paths with.
fn below_crate(names: &str) -> Option<String> {
    names
        .split_once("::")
        .map(|(_, below)| format!("crate::{below}"))
}

/// Whether `module` is the path itself or an ancestor of it.
fn covers(module: &str, path: &str) -> bool {
    path == module
        || path
            .strip_prefix(module)
            .is_some_and(|rest| rest.starts_with("::"))
}
