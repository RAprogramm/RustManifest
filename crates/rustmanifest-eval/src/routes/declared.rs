// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The route table a project declares for `OpenAPI` generation, read as
//! ground truth.
//!
//! A `#[utoipa::path(...)]` attribute names the method and the path of one
//! endpoint next to its handler, written by hand and independent of the
//! router chain the engine walks. That independence is what makes it usable
//! as ground truth: the two tables are produced by different means from the
//! same source, so where they disagree, one of them is wrong.
//!
//! Only the two fields the recovered table also carries are read. Anything
//! the attribute cannot state plainly — a path built at runtime, a method
//! list behind a macro — is left out rather than guessed at, so the ground
//! truth stays a set of facts and not an interpretation.

use std::collections::BTreeSet;

use rustmanifest_engine::Source;

/// One endpoint a project declares, as method and path.
pub type Endpoint = (String, String);

/// Reads every endpoint declared by a `#[utoipa::path]` attribute in
/// `sources`.
///
/// Returns a set, because the ground truth is "which endpoints exist", not
/// "how many times each was written".
#[must_use]
pub(super) fn endpoints(sources: &[Source]) -> BTreeSet<Endpoint> {
    let mut found = BTreeSet::new();
    for source in sources {
        let Some(file) = source.tree() else {
            continue;
        };
        for attribute in attributes(file) {
            if let Some(endpoint) = endpoint_of(&attribute) {
                found.insert(endpoint);
            }
        }
    }
    found
}

/// Collects the token text of every `#[utoipa::path(...)]` attribute in
/// `file`, including attributes on items of nested inline modules.
fn attributes(file: &syn::File) -> Vec<String> {
    let mut items = Vec::new();
    flattened(&file.items, &mut items);
    let mut texts = Vec::new();
    for item in items {
        let syn::Item::Fn(function) = item else {
            continue;
        };
        for attribute in &function.attrs {
            let path = attribute.path();
            let named_utoipa = path.segments.len() == 2
                && path.segments[0].ident == "utoipa"
                && path.segments[1].ident == "path";
            if named_utoipa && let syn::Meta::List(list) = &attribute.meta {
                texts.push(list.tokens.to_string());
            }
        }
    }
    texts
}

/// Appends `items` and the items of every nested inline module to `out`.
fn flattened<'a>(items: &'a [syn::Item], out: &mut Vec<&'a syn::Item>) {
    for item in items {
        out.push(item);
        if let syn::Item::Mod(module) = item
            && let Some((_, nested)) = &module.content
        {
            flattened(nested, out);
        }
    }
}

/// Reads the method and path out of one attribute's token text.
///
/// The method is the bare word the attribute opens with; the path is the
/// string literal assigned to `path`. An attribute missing either states no
/// endpoint and contributes none.
fn endpoint_of(tokens: &str) -> Option<Endpoint> {
    let method = tokens.split(',').next()?.trim().to_ascii_uppercase();
    if method.is_empty() || !method.chars().all(|letter| letter.is_ascii_alphabetic()) {
        return None;
    }
    let after = tokens.split("path = \"").nth(1)?;
    let path = after.split('"').next()?;
    (!path.is_empty()).then(|| (method, path.to_owned()))
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn source(text: &str) -> Vec<Source> {
        vec![Source::new(PathBuf::from("src/lib.rs"), text.to_owned())]
    }

    #[test]
    fn an_annotated_handler_declares_its_method_and_path() {
        let found = endpoints(&source(
            "#[utoipa::path(post, path = \"/api/auth/logout\", tag = \"Auth\")]\npub async fn \
             logout() {}"
        ));
        assert_eq!(
            found.into_iter().collect::<Vec<_>>(),
            vec![("POST".to_owned(), "/api/auth/logout".to_owned())]
        );
    }

    #[test]
    fn the_same_endpoint_written_twice_is_one_endpoint() {
        let found = endpoints(&source(
            "#[utoipa::path(get, path = \"/a\")]\nfn one() {}\n#[utoipa::path(get, path = \
             \"/a\")]\nfn two() {}"
        ));
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn an_attribute_without_a_path_declares_nothing() {
        assert!(endpoints(&source("#[utoipa::path(get)]\nfn bare() {}")).is_empty());
    }

    #[test]
    fn an_unrelated_attribute_declares_nothing() {
        assert!(endpoints(&source("#[test]\nfn checks() {}")).is_empty());
    }

    #[test]
    fn a_handler_inside_an_inline_module_is_read() {
        let found = endpoints(&source(
            "mod rest { #[utoipa::path(delete, path = \"/x\")] pub fn drop_it() {} }"
        ));
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn text_that_is_not_rust_declares_nothing() {
        assert!(endpoints(&source("this is not rust {{{")).is_empty());
    }
}
