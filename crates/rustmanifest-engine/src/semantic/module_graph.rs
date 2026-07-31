// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Module graph of the analyzed workspace slice: crate roots, `mod`
//! resolution, module paths, and expanded `use` targets.
//!
//! The graph is built purely from the [`Source`] slice, never from the
//! filesystem: a `mod name;` declaration resolves to the source whose path
//! is `name.rs` or `name/mod.rs` relative to the declaring file's
//! directory, and inline `mod name { .. }` blocks recurse in place. Every
//! reachable module receives a path of the form `crate`, `crate::a`,
//! `crate::a::b`.
//!
//! Crate roots are sources whose path ends in `src/lib.rs` or
//! `src/main.rs`. Sources not reachable from any crate root — binaries
//! under `src/bin/`, integration tests, files whose `mod` chain is broken —
//! are not part of the graph and are therefore invisible to module-level
//! checks. When the slice contains no crate root at all (a lone fixture
//! file or a single reviewed file), every source becomes its own crate
//! root, so single-file analysis still sees the file's inline module tree.

use std::{
    collections::HashMap,
    path::{Path, PathBuf}
};

use rustmanifest_schema::Range;
use syn::{UseTree, spanned::Spanned};

use crate::source::Source;

/// One expanded `use` target recorded in a module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct UseTarget {
    /// Flat `::`-joined path as written, without a leading `::`.
    pub(super) target: String,
    /// Byte range of the `use` item that declared the target.
    pub(super) range:  Range
}

/// One module reachable from a crate root of the source slice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ModuleNode {
    /// Index of the crate root the module belongs to.
    pub(super) krate:  usize,
    /// Module path, `crate` for a crate root.
    pub(super) path:   String,
    /// Index of the source the module's items are written in.
    pub(super) source: usize,
    /// Expanded `use` targets in declaration order.
    pub(super) uses:   Vec<UseTarget>
}

/// All modules reachable from the crate roots of the source slice.
#[derive(Debug, Default)]
pub(super) struct ModuleGraph {
    modules: Vec<ModuleNode>
}

impl ModuleGraph {
    /// Builds the graph by walking every crate root of `sources`; files
    /// that do not parse as Rust contribute no modules.
    pub(super) fn from_sources(sources: &[Source]) -> Self {
        let mut roots: Vec<usize> = (0..sources.len())
            .filter(|&index| is_crate_root(sources[index].path()))
            .collect();
        if roots.is_empty() {
            roots = (0..sources.len()).collect();
        }
        let mut builder = GraphBuilder {
            sources,
            by_path: sources
                .iter()
                .enumerate()
                .map(|(index, source)| (source.path(), index))
                .collect(),
            modules: Vec::new()
        };
        for (krate, &root) in roots.iter().enumerate() {
            let dir = sources[root]
                .path()
                .parent()
                .map_or_else(PathBuf::new, Path::to_path_buf);
            builder.extract_file(krate, "crate".to_owned(), root, dir);
        }
        Self {
            modules: builder.modules
        }
    }

    /// Modules in deterministic root-then-declaration order.
    pub(super) fn modules(&self) -> &[ModuleNode] {
        &self.modules
    }
}

/// Returns true when `path` names a crate root file (`src/lib.rs` or
/// `src/main.rs`).
fn is_crate_root(path: &Path) -> bool {
    path.ends_with("src/lib.rs") || path.ends_with("src/main.rs")
}

/// Mutable state shared by the recursive module walk.
struct GraphBuilder<'a> {
    /// Source slice the graph is built from.
    sources: &'a [Source],
    /// Source index by file path, for `mod name;` resolution.
    by_path: HashMap<&'a Path, usize>,
    /// Modules collected so far, in discovery order.
    modules: Vec<ModuleNode>
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
    fn extract_file(&mut self, krate: usize, path: String, source: usize, dir: PathBuf) {
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
            uses: Vec::new()
        });
        index
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
        } else if let Some(source) = self.child_source(&scope.dir, &name) {
            self.extract_file(scope.krate, path, source, dir);
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

/// Expands a use tree into flat path strings without a leading `::`.
fn expand_use(prefix: String, tree: &UseTree, out: &mut Vec<String>) {
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
    use std::path::PathBuf;

    use syn::ItemUse;

    use super::{ModuleGraph, expand_use};
    use crate::source::Source;

    fn source(path: &str, text: &str) -> Source {
        Source::new(PathBuf::from(path), text.to_owned())
    }

    fn paths(graph: &ModuleGraph) -> Vec<String> {
        graph
            .modules()
            .iter()
            .map(|module| module.path.clone())
            .collect()
    }

    fn expanded(text: &str) -> Vec<String> {
        let item: ItemUse = syn::parse_str(text).expect("use item");
        let mut out = Vec::new();
        expand_use(String::new(), &item.tree, &mut out);
        out
    }

    #[test]
    fn mod_declaration_resolves_flat_file() {
        let sources = [
            source("src/lib.rs", "mod util;"),
            source("src/util.rs", "use crate::Item;")
        ];
        let graph = ModuleGraph::from_sources(&sources);
        assert_eq!(paths(&graph), vec!["crate", "crate::util"]);
        assert_eq!(graph.modules()[1].source, 1);
        assert_eq!(graph.modules()[1].uses.len(), 1);
        assert_eq!(graph.modules()[1].uses[0].target, "crate::Item");
    }

    #[test]
    fn mod_declaration_resolves_directory_module() {
        let sources = [
            source("src/lib.rs", "mod nested;"),
            source("src/nested/mod.rs", "mod deep;"),
            source("src/nested/deep.rs", "")
        ];
        let graph = ModuleGraph::from_sources(&sources);
        assert_eq!(
            paths(&graph),
            vec!["crate", "crate::nested", "crate::nested::deep"]
        );
    }

    #[test]
    fn inline_modules_recurse() {
        let sources = [source(
            "src/lib.rs",
            "mod outer { mod inner { use std::fmt; } }"
        )];
        let graph = ModuleGraph::from_sources(&sources);
        assert_eq!(
            paths(&graph),
            vec!["crate", "crate::outer", "crate::outer::inner"]
        );
        assert_eq!(graph.modules()[2].uses[0].target, "std::fmt");
    }

    #[test]
    fn unreachable_file_is_ignored() {
        let sources = [
            source("src/lib.rs", "mod util;"),
            source("src/util.rs", ""),
            source("src/orphan.rs", "use crate::util::Item;")
        ];
        let graph = ModuleGraph::from_sources(&sources);
        assert_eq!(paths(&graph), vec!["crate", "crate::util"]);
    }

    #[test]
    fn every_file_is_a_root_when_no_crate_root_exists() {
        let sources = [
            source("rules/fixture.rs", "mod a { use std::fmt; }"),
            source("rules/other.rs", "")
        ];
        let graph = ModuleGraph::from_sources(&sources);
        assert_eq!(paths(&graph), vec!["crate", "crate::a", "crate"]);
        assert_eq!(graph.modules()[0].krate, 0);
        assert_eq!(graph.modules()[2].krate, 1);
    }

    #[test]
    fn main_rs_is_a_crate_root() {
        let sources = [source("src/main.rs", "mod cli;"), source("src/cli.rs", "")];
        let graph = ModuleGraph::from_sources(&sources);
        assert_eq!(paths(&graph), vec!["crate", "crate::cli"]);
    }

    #[test]
    fn unparsable_file_contributes_no_modules() {
        let sources = [
            source("src/lib.rs", "mod broken;"),
            source("src/broken.rs", "fn broken(")
        ];
        let graph = ModuleGraph::from_sources(&sources);
        assert_eq!(paths(&graph), vec!["crate"]);
    }

    #[test]
    fn use_ranges_span_the_declaring_item() {
        let sources = [source("src/lib.rs", "use std::fmt;")];
        let graph = ModuleGraph::from_sources(&sources);
        let target = &graph.modules()[0].uses[0];
        assert_eq!(target.range.start, 0);
        assert_eq!(target.range.end, 13);
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

    #[test]
    fn graph_construction_is_deterministic() {
        let sources = [
            source("src/lib.rs", "mod a; mod b { use crate::a::Item; }"),
            source("src/a.rs", "use std::fmt;")
        ];
        let first = ModuleGraph::from_sources(&sources);
        let second = ModuleGraph::from_sources(&sources);
        assert_eq!(first.modules(), second.modules());
    }
}
