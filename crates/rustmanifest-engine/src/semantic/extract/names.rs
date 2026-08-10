// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The name vocabulary of the extraction: which identifiers a piece of
//! text mentions, and which names one `use` tree binds.

use std::collections::BTreeSet;

use proc_macro2::{TokenStream, TokenTree};

/// Every identifier the text mentions, read from its token stream so
/// strings and comments never count as mentions.
pub(super) fn mentioned_idents(text: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    if let Ok(stream) = text.parse::<TokenStream>() {
        collect(stream, &mut found);
    }
    found
}

/// One token tree level of [`mentioned_idents`].
fn collect(stream: TokenStream, found: &mut BTreeSet<String>) {
    for token in stream {
        match token {
            TokenTree::Group(group) => collect(group.stream(), found),
            TokenTree::Ident(ident) => {
                found.insert(ident.to_string());
            }
            TokenTree::Literal(_) | TokenTree::Punct(_) => {}
        }
    }
}

/// The names one `use` tree binds in the file's namespace: leaves and
/// renames, groups walked whole. A glob binds no trackable name, and an
/// underscore rename (`as _`) brings a trait into scope namelessly —
/// both contribute nothing, and the caller treats an empty answer as
/// "always keep": a silent copy is a smaller claim than a broken build.
pub(super) fn bound_names(tree: &syn::UseTree) -> Vec<String> {
    let mut found = Vec::new();
    let nameless = gather(tree, &mut found);
    if nameless { Vec::new() } else { found }
}

/// One `use` tree level of [`bound_names`]; answers whether a nameless
/// binding was seen anywhere in the tree.
fn gather(tree: &syn::UseTree, found: &mut Vec<String>) -> bool {
    match tree {
        syn::UseTree::Path(path) => gather(&path.tree, found),
        syn::UseTree::Name(name) => {
            found.push(name.ident.to_string());
            false
        }
        syn::UseTree::Rename(renamed) => {
            if renamed.rename == "_" {
                return true;
            }
            found.push(renamed.rename.to_string());
            false
        }
        syn::UseTree::Group(group) => {
            let mut nameless = false;
            for item in &group.items {
                nameless |= gather(item, found);
            }
            nameless
        }
        syn::UseTree::Glob(_) => true
    }
}
