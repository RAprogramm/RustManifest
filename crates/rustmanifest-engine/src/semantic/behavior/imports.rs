// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Import table of one file: every name a `use` declaration brings into
//! scope mapped to the full path it resolves to, so a bare call can be
//! classified by what it really names.

/// Maps every name a file imports to its full `use` path, across all
/// modules of the file — file-level granularity is a documented
/// approximation. Glob imports stay out; a name imported twice with
/// different targets is dropped so expansion never guesses.
pub(super) fn use_imports(items: &[&syn::Item]) -> std::collections::BTreeMap<String, String> {
    let mut table: std::collections::BTreeMap<String, Option<String>> =
        std::collections::BTreeMap::new();
    for item in items {
        if let syn::Item::Use(declaration) = item {
            collect_use_tree(&declaration.tree, &mut Vec::new(), &mut table);
        }
    }
    settled(table)
}

/// Walks one `use` tree, recording every imported leaf under its bound
/// name with the full path it resolves to.
fn collect_use_tree(
    tree: &syn::UseTree,
    prefix: &mut Vec<String>,
    table: &mut std::collections::BTreeMap<String, Option<String>>
) {
    match tree {
        syn::UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            collect_use_tree(&path.tree, prefix, table);
            prefix.pop();
        }
        syn::UseTree::Name(name) => {
            let leaf = name.ident.to_string();
            let full = if leaf == "self" {
                prefix.join("::")
            } else {
                let mut segments = prefix.clone();
                segments.push(leaf.clone());
                segments.join("::")
            };
            let key = if leaf == "self" {
                prefix.last().cloned().unwrap_or(leaf)
            } else {
                leaf
            };
            note_unique(table, key, full);
        }
        syn::UseTree::Rename(renamed) => {
            let mut segments = prefix.clone();
            segments.push(renamed.ident.to_string());
            note_unique(table, renamed.rename.to_string(), segments.join("::"));
        }
        syn::UseTree::Group(group) => {
            for entry in &group.items {
                collect_use_tree(entry, prefix, table);
            }
        }
        syn::UseTree::Glob(_) => {}
    }
}

/// Records `value` under `key`, dropping the key on conflicting
/// observations so typing through the table never guesses.
fn note_unique(
    table: &mut std::collections::BTreeMap<String, Option<String>>,
    key: String,
    value: String
) {
    table
        .entry(key)
        .and_modify(|known| {
            if known.as_deref() != Some(value.as_str()) {
                *known = None;
            }
        })
        .or_insert(Some(value));
}

/// Keeps only the keys that settled on exactly one observation.
fn settled(
    table: std::collections::BTreeMap<String, Option<String>>
) -> std::collections::BTreeMap<String, String> {
    table
        .into_iter()
        .filter_map(|(key, value)| value.map(|v| (key, v)))
        .collect()
}
