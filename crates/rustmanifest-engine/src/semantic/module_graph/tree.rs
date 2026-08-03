// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Recursive module-tree construction: walks every crate root, resolves
//! `mod name;` declarations to files, records inline modules, and
//! expands `use` trees into flat targets.

mod mod_source;
mod use_path;

use std::{
    collections::HashMap,
    path::{Path, PathBuf}
};

use rustmanifest_schema::Range;
use syn::spanned::Spanned;

use self::{mod_source::declared_source, use_path::expand_use};
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

        if let Some((_, items)) = &item.content {
            let module = self.push_module(scope.krate, path.clone(), scope.source);
            let child = ModuleScope {
                krate: scope.krate,
                module,
                path,
                source: scope.source,
                dir: scope.dir.join(&name)
            };
            self.extract_items(&child, items);
        } else if let Some((source, source_dir)) =
            declared_source(&self.by_path, &scope.dir, item, &name)
        {
            self.extract_file(scope.krate, path, source, source_dir);
        }
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
