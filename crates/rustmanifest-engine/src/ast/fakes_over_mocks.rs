// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! `fakes-over-mocks` check: tests use hand-written fakes with real
//! behavior, not mock libraries (`STRUCTURE.md` §9).
//!
//! Reports `use` items whose path starts with `mockall`, `mockito`, or
//! `mockiato`, and `#[automock]` / `#[mockall::automock]` attributes on
//! items.

use syn::spanned::Spanned;

use super::{Occurrence, flattened};

/// Mock library crates whose imports are reported.
const MOCK_CRATES: [&str; 3] = ["mockall", "mockito", "mockiato"];

/// Reports mock-library imports and `#[automock]` attributes.
pub(super) fn occurrences(file: &syn::File) -> Vec<Occurrence> {
    let mut items = Vec::new();
    flattened(&file.items, &mut items);

    let mut found = Vec::new();
    for item in items {
        if let syn::Item::Use(import) = item {
            if let Some(root) = root_ident(&import.tree) {
                let name = root.to_string();
                if MOCK_CRATES.contains(&name.as_str()) {
                    found.push(Occurrence::new(
                        &import.span(),
                        format!(
                            "import of mock library `{name}`; prefer a hand-written fake with \
                             real behavior"
                        ),
                    ));
                }
            }
            continue;
        }
        for attr in attributes(item) {
            if automock(attr) {
                found.push(Occurrence::new(
                    &attr.span(),
                    "`#[automock]` generates a mock; prefer a hand-written fake with real \
                     behavior"
                        .to_owned(),
                ));
            }
        }
    }
    found
}

/// Returns the first path component of a use tree, when it has one.
const fn root_ident(tree: &syn::UseTree) -> Option<&syn::Ident> {
    match tree {
        syn::UseTree::Path(path) => Some(&path.ident),
        syn::UseTree::Name(name) => Some(&name.ident),
        syn::UseTree::Rename(rename) => Some(&rename.ident),
        syn::UseTree::Glob(_) | syn::UseTree::Group(_) => None,
    }
}

/// Returns the attributes of item kinds that can carry `#[automock]`.
fn attributes(item: &syn::Item) -> &[syn::Attribute] {
    match item {
        syn::Item::Trait(definition) => &definition.attrs,
        syn::Item::Struct(definition) => &definition.attrs,
        syn::Item::Enum(definition) => &definition.attrs,
        syn::Item::Fn(function) => &function.attrs,
        syn::Item::Impl(block) => &block.attrs,
        syn::Item::Mod(module) => &module.attrs,
        _ => &[],
    }
}

/// True for `#[automock]` and `#[mockall::automock]`.
fn automock(attr: &syn::Attribute) -> bool {
    let path = attr.path();
    if path.is_ident("automock") {
        return true;
    }
    let mut segments = path.segments.iter();
    matches!(
        (segments.next(), segments.next(), segments.next()),
        (Some(first), Some(second), None)
            if first.ident == "mockall" && second.ident == "automock"
    )
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use super::occurrences;

    fn count(source: &str) -> usize {
        occurrences(&syn::parse_file(source).unwrap()).len()
    }

    #[test]
    fn mock_crate_imports_are_reported() {
        assert_eq!(count("use mockall::automock;"), 1);
        assert_eq!(count("use mockito::Server;"), 1);
        assert_eq!(count("use mockiato::mockable;"), 1);
    }

    #[test]
    fn automock_attribute_is_reported() {
        assert_eq!(count("#[automock] trait Storage { fn load(&self); }"), 1);
        assert_eq!(
            count("#[mockall::automock] trait Storage { fn load(&self); }"),
            1
        );
    }

    #[test]
    fn ordinary_imports_pass() {
        assert_eq!(count("use std::collections::HashMap;"), 0);
        assert_eq!(count("use mockallish::thing;"), 0);
    }

    #[test]
    fn ordinary_attributes_pass() {
        assert_eq!(count("#[derive(Debug)] struct Fake;"), 0);
        assert_eq!(count("#[automock_extra] trait T {}"), 0);
    }

    #[test]
    fn nested_module_imports_are_reported() {
        assert_eq!(count("mod tests { use mockall::automock; }"), 1);
    }
}
