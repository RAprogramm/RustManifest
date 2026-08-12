// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The use targets one file writes.
//!
//! Only the imports are read from the text now. Where a module lives is
//! the compiler's answer, carried by a resolution; what a file imports is
//! written in the file and nowhere else, so it is read here — grouped
//! trees expanded, each leaf recorded with the range it was written at.

use rustmanifest_schema::Range;
use syn::spanned::Spanned as _;

use super::UseTarget;

/// Records one use target per imported path of `items`, expanding grouped
/// trees and reporting the module path each import is written in.
///
/// An inline module writes its own imports, and they belong to it and not
/// to the file's outermost module — a cycle between two inline siblings is
/// invisible to a reading that pools them.
pub(super) fn use_targets(items: &[syn::Item], inside: &str, into: &mut Vec<(String, UseTarget)>) {
    for item in items {
        match item {
            syn::Item::Use(imported) => {
                let bytes = imported.span().byte_range();
                let range = Range {
                    start: u32::try_from(bytes.start).unwrap_or(u32::MAX),
                    end:   u32::try_from(bytes.end).unwrap_or(u32::MAX)
                };
                let mut written = Vec::new();
                expanded(&imported.tree, "", &mut written);
                into.extend(written.into_iter().map(|target| {
                    (
                        inside.to_owned(),
                        UseTarget {
                            target,
                            range
                        }
                    )
                }));
            }
            syn::Item::Mod(held) => {
                if let Some((_, inner)) = &held.content {
                    use_targets(inner, &format!("{inside}::{}", held.ident), into);
                }
            }
            _ => {}
        }
    }
}

/// Expands one use tree into the full paths it imports.
fn expanded(tree: &syn::UseTree, prefix: &str, into: &mut Vec<String>) {
    let joined = |name: &str| {
        if prefix.is_empty() {
            name.to_owned()
        } else {
            format!("{prefix}::{name}")
        }
    };
    match tree {
        syn::UseTree::Path(held) => {
            expanded(&held.tree, &joined(&held.ident.to_string()), into);
        }
        syn::UseTree::Name(held) => into.push(joined(&held.ident.to_string())),
        syn::UseTree::Rename(held) => into.push(joined(&held.ident.to_string())),
        syn::UseTree::Glob(_) => into.push(joined("*")),
        syn::UseTree::Group(held) => {
            for inner in &held.items {
                expanded(inner, prefix, into);
            }
        }
    }
}
