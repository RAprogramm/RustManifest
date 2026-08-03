// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The source a `mod name;` declaration mounts: an explicit
//! `#[path = "..."]` attribute first, then the `name.rs` and
//! `name/mod.rs` conventions relative to the declaring file's directory.

use std::{
    collections::HashMap,
    path::{Path, PathBuf}
};

/// Resolves the source and the directory its own child modules resolve
/// against for a `mod name;` declaration written in `dir`; declarations
/// naming no known source yield [`None`].
pub(super) fn declared_source(
    by_path: &HashMap<&Path, usize>,
    dir: &Path,
    item: &syn::ItemMod,
    name: &str
) -> Option<(usize, PathBuf)> {
    path_attribute(item)
        .and_then(|relative| {
            let file = dir.join(relative);
            let parent = file.parent().map(Path::to_path_buf);
            by_path.get(file.as_path()).copied().zip(parent)
        })
        .or_else(|| child_source(by_path, dir, name).map(|source| (source, dir.join(name))))
}

/// Resolves the source of a `mod name;` declaration, trying `name.rs`
/// then `name/mod.rs` relative to `dir`; unresolved declarations are
/// ignored.
fn child_source(by_path: &HashMap<&Path, usize>, dir: &Path, name: &str) -> Option<usize> {
    let flat = dir.join(format!("{name}.rs"));
    if let Some(&index) = by_path.get(flat.as_path()) {
        return Some(index);
    }
    let nested = dir.join(name).join("mod.rs");
    by_path.get(nested.as_path()).copied()
}

/// Extracts the string of a `#[path = "..."]` attribute on a `mod`
/// declaration, which mounts the module from an explicit file relative to
/// the declaring file's directory.
fn path_attribute(item: &syn::ItemMod) -> Option<String> {
    item.attrs.iter().find_map(|attr| {
        if !attr.path().is_ident("path") {
            return None;
        }
        let syn::Meta::NameValue(pair) = &attr.meta else {
            return None;
        };
        let syn::Expr::Lit(literal) = &pair.value else {
            return None;
        };
        let syn::Lit::Str(text) = &literal.lit else {
            return None;
        };
        Some(text.value())
    })
}
