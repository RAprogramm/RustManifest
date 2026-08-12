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
                let mut written = Vec::new();
                expanded(&imported.tree, "", &mut written);
                into.extend(written.into_iter().map(|(target, range)| {
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

/// Expands one use tree into the full paths it imports, each with the
/// range of the leaf that names it.
///
/// The leaf's own range, not the whole item's: a resolution answers what a
/// written name reaches by the place it is written at, and a grouped
/// import writes several names in one item.
fn expanded(tree: &syn::UseTree, prefix: &str, into: &mut Vec<(String, Range)>) {
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
        syn::UseTree::Name(held) => {
            into.push((joined(&held.ident.to_string()), ranged(held.ident.span())));
        }
        syn::UseTree::Rename(held) => {
            into.push((joined(&held.ident.to_string()), ranged(held.ident.span())));
        }
        syn::UseTree::Glob(held) => {
            into.push((joined("*"), ranged(held.star_token.span())));
        }
        syn::UseTree::Group(held) => {
            for inner in &held.items {
                expanded(inner, prefix, into);
            }
        }
    }
}

/// The byte range one span covers.
fn ranged(span: proc_macro2::Span) -> Range {
    let bytes = span.byte_range();
    Range {
        start: u32::try_from(bytes.start).unwrap_or(u32::MAX),
        end:   u32::try_from(bytes.end).unwrap_or(u32::MAX)
    }
}
