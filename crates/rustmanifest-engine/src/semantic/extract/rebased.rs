// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Carried imports rebased one module deeper: a `use` line whose first
//! segment names the parent itself — `self`, or a module the parent
//! declares, resolved by uniform paths — gains a `super::` step so the
//! same name still resolves from the child.
//!
//! The corpus judge caught the unrebased spelling: a parent's
//! `use error::ReadmeError;` copied verbatim into the child is an
//! unresolved import, because the child declares no `error` of its
//! own.

use std::collections::BTreeSet;

/// The module names the parent file declares.
pub(super) fn declared_modules(parsed: &syn::File) -> BTreeSet<String> {
    parsed
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Mod(module) => Some(module.ident.to_string()),
            _ => None
        })
        .collect()
}

/// The written `use` line with its first segment stepped through
/// `super` where the segment resolves inside the parent — unchanged
/// where the path is absolute. A line already stepping through
/// `super` gains one more step, because the child stands one module
/// deeper than the parent the line was written for — the corpus judge
/// caught the unstepped copy as an unresolved import.
pub(super) fn rebased(written: &str, used: &syn::ItemUse, locals: &BTreeSet<String>) -> String {
    let Some(first) = first_segment(&used.tree) else {
        return written.to_owned();
    };
    if first == "self" {
        return written.replacen("use self::", "use super::", 1);
    }
    if first == "super" {
        return written.replacen("use super::", "use super::super::", 1);
    }
    if locals.contains(&first) {
        return written.replacen(
            &format!("use {first}::"),
            &format!("use super::{first}::"),
            1
        );
    }
    written.to_owned()
}

/// The first path segment of a `use` tree, where one is written.
fn first_segment(tree: &syn::UseTree) -> Option<String> {
    match tree {
        syn::UseTree::Path(path) => Some(path.ident.to_string()),
        syn::UseTree::Name(name) => Some(name.ident.to_string()),
        syn::UseTree::Rename(renamed) => Some(renamed.ident.to_string()),
        syn::UseTree::Glob(_) | syn::UseTree::Group(_) => None
    }
}
