// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The module names a call may spell in front of a function.
//!
//! A free function answers to the scope that declares it and to nothing
//! else, so the names collected here are read from the declaring source
//! itself: the innermost inline module around the function, or the
//! module a file is at its top level. A function whose scope could not
//! be read keeps an empty set, which lets a qualified candidate fail to
//! bind rather than bind by proximity of spelling.

use std::collections::{BTreeMap, BTreeSet};

use crate::{semantic::behavior::FunctionFacts, source::Source};

/// Names that may qualify each function in a call, by fact index.
///
/// A free function answers to the scope that declares it and to nothing
/// else: the innermost inline module around it, or — at a file's top
/// level — the module the file itself is, which is its stem, its
/// directory for a `mod.rs`, and the crate for a crate root. A function
/// no scope could be read for keeps an empty set, so a qualified
/// candidate never binds to it by proximity of spelling alone.
pub(super) fn qualifiers_by_fact(
    sources: &[Source],
    facts: &[FunctionFacts]
) -> Vec<BTreeSet<String>> {
    let mut positions: BTreeMap<(usize, u32, u32), usize> = BTreeMap::new();
    for (index, fact) in facts.iter().enumerate() {
        positions.insert((fact.source, fact.range.start, fact.range.end), index);
    }
    let mut qualifiers = vec![BTreeSet::new(); facts.len()];
    for (index, source) in sources.iter().enumerate() {
        let Ok(file) = syn::parse_file(source.text()) else {
            continue;
        };
        let scope = file_scope(&source.path().to_string_lossy());
        scope_items(&file.items, &scope, index, &positions, &mut qualifiers);
    }
    qualifiers
}

/// Records the scope of every function declared directly in `items`,
/// descending into inline modules, which replace the scope for what they
/// hold.
fn scope_items(
    items: &[syn::Item],
    scope: &BTreeSet<String>,
    source: usize,
    positions: &BTreeMap<(usize, u32, u32), usize>,
    qualifiers: &mut [BTreeSet<String>]
) {
    for item in items {
        match item {
            syn::Item::Fn(function) => {
                let span = function.sig.ident.span().byte_range();
                let start = u32::try_from(span.start).unwrap_or(u32::MAX);
                let end = u32::try_from(span.end).unwrap_or(u32::MAX);
                if let Some(index) = positions.get(&(source, start, end))
                    && let Some(slot) = qualifiers.get_mut(*index)
                {
                    slot.clone_from(scope);
                }
            }
            syn::Item::Mod(module) => {
                if let Some((_, nested)) = &module.content {
                    let inner: BTreeSet<String> =
                        std::iter::once(module.ident.to_string()).collect();
                    scope_items(nested, &inner, source, positions, qualifiers);
                }
            }
            _ => {}
        }
    }
}

/// The module names a file's own top level answers to.
fn file_scope(path: &str) -> BTreeSet<String> {
    let normalized = path.replace('\\', "/");
    let components: Vec<&str> = normalized
        .split('/')
        .filter(|part| !part.is_empty())
        .collect();
    let mut names = BTreeSet::new();
    let Some((last, directories)) = components.split_last() else {
        return names;
    };
    match last.strip_suffix(".rs").unwrap_or(last) {
        "mod" => {
            if let Some(parent) = directories.last() {
                names.insert((*parent).to_owned());
            }
        }
        "lib" | "main" => {
            names.insert("crate".to_owned());
            if let Some(krate) = crate_name(directories) {
                names.insert(krate);
            }
        }
        stem => {
            names.insert(stem.to_owned());
        }
    }
    names
}

/// The crate a source directory belongs to, read from the `<crate>/src/`
/// layout with `-` normalized to `_` as the module system spells it.
fn crate_name(directories: &[&str]) -> Option<String> {
    let at = directories.iter().position(|part| *part == "src")?;
    let owner = at.checked_sub(1).and_then(|index| directories.get(index))?;
    Some(owner.replace('-', "_"))
}
