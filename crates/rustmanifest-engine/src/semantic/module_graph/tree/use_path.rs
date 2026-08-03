// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The flat paths a `use` tree names: grouped braces expand into one path
//! per leaf, a glob becomes a `*` leaf, a rename keeps the original path,
//! and `self` inside a group targets the prefix itself.

use syn::UseTree;

/// Expands a use tree into flat path strings without a leading `::`.
pub(super) fn expand_use(prefix: String, tree: &UseTree, out: &mut Vec<String>) {
    match tree {
        UseTree::Path(segment) => {
            expand_use(
                joined(&prefix, &segment.ident.to_string()),
                &segment.tree,
                out
            );
        }
        UseTree::Name(name) => {
            let ident = name.ident.to_string();
            if ident == "self" && !prefix.is_empty() {
                out.push(prefix);
            } else {
                out.push(joined(&prefix, &ident));
            }
        }
        UseTree::Rename(rename) => out.push(joined(&prefix, &rename.ident.to_string())),
        UseTree::Glob(_) => out.push(joined(&prefix, "*")),
        UseTree::Group(group) => {
            for item in &group.items {
                expand_use(prefix.clone(), item, out);
            }
        }
    }
}

/// Joins a path prefix and a segment with `::`, dropping an empty prefix.
fn joined(prefix: &str, segment: &str) -> String {
    if prefix.is_empty() {
        segment.to_string()
    } else {
        format!("{prefix}::{segment}")
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use syn::ItemUse;

    use super::expand_use;

    fn expanded(text: &str) -> Vec<String> {
        let item: ItemUse = syn::parse_str(text).expect("use item");
        let mut out = Vec::new();
        expand_use(String::new(), &item.tree, &mut out);
        out
    }

    #[test]
    fn group_expands_into_separate_targets() {
        assert_eq!(
            expanded("use a::{b, c};"),
            vec!["a::b".to_string(), "a::c".to_string()]
        );
    }

    #[test]
    fn glob_renders_as_star() {
        assert_eq!(expanded("use a::b::*;"), vec!["a::b::*".to_string()]);
    }

    #[test]
    fn leading_colon_is_dropped() {
        assert_eq!(expanded("use ::std::fmt;"), vec!["std::fmt".to_string()]);
    }

    #[test]
    fn self_in_group_targets_the_prefix() {
        assert_eq!(
            expanded("use a::b::{self, c};"),
            vec!["a::b".to_string(), "a::b::c".to_string()]
        );
    }

    #[test]
    fn rename_records_the_original_path() {
        assert_eq!(expanded("use a::b as c;"), vec!["a::b".to_string()]);
    }
}
