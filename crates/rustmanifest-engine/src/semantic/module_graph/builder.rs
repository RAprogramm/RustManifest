// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Recursive module-tree construction: walks every crate root, resolves
//! `mod name;` declarations to files, records inline modules, and
//! expands `use` trees into flat targets.

use std::{
    collections::HashMap,
    path::{Path, PathBuf}
};

use rustmanifest_schema::Range;
use syn::{UseTree, spanned::Spanned};

use super::{ModuleNode, UseTarget};
use crate::source::Source;

pub(super) struct GraphBuilder<'a> {
    /// Source slice the graph is built from.
    pub(super) sources: &'a [Source],
    /// Source index by file path, for `mod name;` resolution.
    pub(super) by_path: HashMap<&'a Path, usize>,
    /// Modules collected so far, in discovery order.
    pub(super) modules: Vec<ModuleNode>,
    /// Whether modules recorded now belong to a layout crate root.
    pub(super) layout:  bool
}

/// Position of the module currently being extracted.
struct ModuleScope {
    /// Crate root index the module belongs to.
    krate:  usize,
    /// Index of the module's [`ModuleNode`] in the builder.
    module: usize,
    /// Module path string, `crate` for the root module.
    path:   String,
    /// Index of the source the module's items are written in.
    source: usize,
    /// Directory child file modules resolve against.
    dir:    PathBuf
}

impl GraphBuilder<'_> {
    /// Parses the source at `source` and extracts its module tree.
    pub(super) fn extract_file(
        &mut self,
        krate: usize,
        path: String,
        source: usize,
        dir: PathBuf
    ) {
        let Some(text) = self.sources.get(source).map(Source::text) else {
            return;
        };
        let Ok(file) = syn::parse_file(text) else {
            return;
        };
        let module = self.push_module(krate, path.clone(), source);
        let scope = ModuleScope {
            krate,
            module,
            path,
            source,
            dir
        };
        self.extract_items(&scope, &file.items);
    }

    /// Records a new module node and returns its index.
    fn push_module(&mut self, krate: usize, path: String, source: usize) -> usize {
        let index = self.modules.len();
        self.modules.push(ModuleNode {
            krate,
            path,
            source,
            layout: self.layout,
            uses: Vec::new()
        });
        index
    }

    /// Returns whether the source at `index` is already part of a module.
    pub(super) fn claimed(&self, index: usize) -> bool {
        self.modules.iter().any(|module| module.source == index)
    }

    /// Extracts `mod` declarations and `use` items of one module body.
    fn extract_items(&mut self, scope: &ModuleScope, items: &[syn::Item]) {
        for item in items {
            match item {
                syn::Item::Mod(declaration) => self.extract_mod(scope, declaration),
                syn::Item::Use(import) => self.extract_use(scope, import),
                _ => {}
            }
        }
    }

    /// Extracts a `mod` item, inline or referring to a separate source.
    fn extract_mod(&mut self, scope: &ModuleScope, item: &syn::ItemMod) {
        let name = item.ident.to_string();
        let path = format!("{}::{name}", scope.path);
        let dir = scope.dir.join(&name);

        if let Some((_, items)) = &item.content {
            let module = self.push_module(scope.krate, path.clone(), scope.source);
            let child = ModuleScope {
                krate: scope.krate,
                module,
                path,
                source: scope.source,
                dir
            };
            self.extract_items(&child, items);
        } else if let Some((source, source_dir)) = path_attribute(item)
            .and_then(|relative| {
                let file = scope.dir.join(relative);
                let parent = file.parent().map(Path::to_path_buf);
                self.by_path
                    .get(file.as_path())
                    .copied()
                    .zip(parent)
            })
            .or_else(|| {
                self.child_source(&scope.dir, &name)
                    .map(|source| (source, dir.clone()))
            })
        {
            self.extract_file(scope.krate, path, source, source_dir);
        }
    }

    /// Resolves the source of a `mod name;` declaration, trying `name.rs`
    /// then `name/mod.rs` relative to `dir`; unresolved declarations are
    /// ignored.
    fn child_source(&self, dir: &Path, name: &str) -> Option<usize> {
        let flat = dir.join(format!("{name}.rs"));
        if let Some(&index) = self.by_path.get(flat.as_path()) {
            return Some(index);
        }
        let nested = dir.join(name).join("mod.rs");
        self.by_path.get(nested.as_path()).copied()
    }

    /// Records one use target per imported path, expanding grouped trees.
    fn extract_use(&mut self, scope: &ModuleScope, item: &syn::ItemUse) {
        let bytes = item.span().byte_range();
        let range = Range {
            start: u32::try_from(bytes.start).unwrap_or(u32::MAX),
            end:   u32::try_from(bytes.end).unwrap_or(u32::MAX)
        };
        let mut targets = Vec::new();
        expand_use(String::new(), &item.tree, &mut targets);
        if let Some(node) = self.modules.get_mut(scope.module) {
            for target in targets {
                node.uses.push(UseTarget {
                    target,
                    range
                });
            }
        }
    }
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
