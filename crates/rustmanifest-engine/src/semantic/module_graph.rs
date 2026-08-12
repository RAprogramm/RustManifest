// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The module tree of the analyzed workspace, as the compiler states it.
//!
//! Each node names the module, the file its body is written in, and the
//! use targets that module writes. Which file holds which module is not
//! decided here: a resolver settles it, because the language decides it by
//! rules a file-name reading cannot follow — a `path` attribute, a
//! configuration flag, a file whose name differs from the module's.
//!
//! Without a settled tree there is no graph. Every reader above treats an
//! absent module as an unanswered question, which is what it is.

mod resolve;
mod tree;

use std::{collections::HashMap, path::Path};

use rustmanifest_schema::Range;

pub(super) use self::resolve::resolved_module;
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
    /// Whether the crate root follows cargo's target layout; `false` for
    /// standalone fallback roots.
    /// Expanded `use` targets in declaration order.
    pub(super) uses:   Vec<UseTarget>
}

/// All modules reachable from the crate roots of the source slice.
#[derive(Debug, Default)]
pub(super) struct ModuleGraph {
    modules: Vec<ModuleNode>
}

impl ModuleGraph {
    /// Builds the graph from the module tree a resolver settled, taking
    /// the uses each file writes from the file itself.
    ///
    /// Which file holds which module is the compiler's answer: a module
    /// mounted by a `path` attribute, one behind a configuration flag, and
    /// one whose file does not match its name are each placed where the
    /// compiler places them. The reading that stood here rebuilt the
    /// mounting out of file names and got all three wrong.
    ///
    /// Without a settled tree the graph is empty. A guessed mounting reads
    /// exactly like a derived one and is not one.
    pub(super) fn of(
        sources: &[Source],
        settled: Option<&rustmanifest_schema::Resolution>
    ) -> Self {
        let Some(resolution) = settled else {
            return Self::default();
        };
        let by_path: HashMap<&Path, usize> = sources
            .iter()
            .enumerate()
            .map(|(index, source)| (source.path(), index))
            .collect();
        let mut crates: Vec<&str> = Vec::new();
        let mut modules = Vec::new();
        for held in &resolution.modules {
            let Some(&source) = by_path.get(held.file.as_path()) else {
                continue;
            };
            let (krate, below) = crate_and_path(&held.names);
            let index = crates
                .iter()
                .position(|named| *named == krate)
                .unwrap_or_else(|| {
                    crates.push(krate);
                    crates.len() - 1
                });
            modules.push(ModuleNode {
                krate: index,
                path: below,
                source,
                uses: Vec::new()
            });
        }
        attach_uses(sources, &mut modules);
        Self {
            modules
        }
    }

    /// Modules in deterministic root-then-declaration order.
    pub(super) fn modules(&self) -> &[ModuleNode] {
        &self.modules
    }
}

/// The crate a settled module path names, and the path inside it with the
/// crate name replaced by the keyword every reader here expects.
fn crate_and_path(names: &str) -> (&str, String) {
    names.split_once("::").map_or_else(
        || (names, "crate".to_owned()),
        |(krate, below)| (krate, format!("crate::{below}"))
    )
}

/// Records the use targets each module writes, in the module they are
/// written in.
fn attach_uses(sources: &[Source], modules: &mut [ModuleNode]) {
    let mut by_place: std::collections::BTreeMap<(usize, String), usize> =
        std::collections::BTreeMap::new();
    for (index, module) in modules.iter().enumerate() {
        by_place
            .entry((module.source, module.path.clone()))
            .or_insert(index);
    }
    let mut outermost: std::collections::BTreeMap<usize, String> =
        std::collections::BTreeMap::new();
    for module in modules.iter() {
        let held = outermost
            .entry(module.source)
            .or_insert_with(|| module.path.clone());
        if held.len() > module.path.len() {
            held.clone_from(&module.path);
        }
    }
    for (source, top) in outermost {
        let Some(text) = sources.get(source).map(Source::text) else {
            continue;
        };
        let Ok(file) = syn::parse_file(text) else {
            continue;
        };
        let mut written = Vec::new();
        tree::use_targets(&file.items, &top, &mut written);
        for (inside, target) in written {
            if let Some(&index) = by_place.get(&(source, inside)) {
                modules[index].uses.push(target);
            }
        }
    }
}

#[cfg(test)]
#[expect(
    unreachable_pub,
    reason = "the fixture builder is reached from test modules outside this one"
)]
pub fn declared_tree(sources: &[Source]) -> rustmanifest_schema::Resolution {
    use std::path::PathBuf;

    use rustmanifest_schema::{Resolution, ResolvedModule};

    let anchored = sources.iter().any(|source| {
        source
            .path()
            .file_stem()
            .and_then(|held| held.to_str())
            .is_some_and(|stem| matches!(stem, "lib" | "main"))
    });
    let mut modules = Vec::new();
    for source in sources {
        let path = source.path();
        let Some(stem) = path.file_stem().and_then(|held| held.to_str()) else {
            continue;
        };
        let root = !anchored || matches!(stem, "lib" | "main");
        let mut segments: Vec<String> = path
            .components()
            .skip_while(|part| part.as_os_str() != "src")
            .skip(1)
            .filter_map(|part| part.as_os_str().to_str().map(ToOwned::to_owned))
            .collect();
        segments.pop();
        let named = if root {
            "fixture".to_owned()
        } else if segments.is_empty() {
            format!("fixture::{stem}")
        } else {
            format!("fixture::{}::{stem}", segments.join("::"))
        };
        modules.push(ResolvedModule {
            names:      named.clone(),
            file:       PathBuf::from(path),
            crate_root: root,
            parent:     named.rsplit_once("::").map(|(above, _)| above.to_owned())
        });
        let Ok(parsed) = syn::parse_file(source.text()) else {
            continue;
        };
        inline_modules(&parsed.items, &named, path, &mut modules);
    }
    let mut resolution = Resolution {
        resolver: "fixture".to_owned(),
        modules,
        ..Resolution::default()
    };
    resolution.settle();
    resolution
}

/// Records every inline module of `items` under `above`, in the file that
/// holds them.
#[cfg(test)]
fn inline_modules(
    items: &[syn::Item],
    above: &str,
    file: &Path,
    into: &mut Vec<rustmanifest_schema::ResolvedModule>
) {
    for item in items {
        let syn::Item::Mod(held) = item else {
            continue;
        };
        let Some((_, inner)) = &held.content else {
            continue;
        };
        let named = format!("{above}::{}", held.ident);
        into.push(rustmanifest_schema::ResolvedModule {
            names:      named.clone(),
            file:       std::path::PathBuf::from(file),
            crate_root: false,
            parent:     Some(above.to_owned())
        });
        inline_modules(inner, &named, file, into);
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

    use super::{ModuleGraph, declared_tree};
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

    #[test]
    fn the_tree_is_taken_from_what_the_resolver_settled() {
        let sources = [
            source("src/lib.rs", "mod util;"),
            source("src/util.rs", "use crate::Item;")
        ];
        let settled = declared_tree(&sources);
        let graph = ModuleGraph::of(&sources, Some(&settled));
        assert_eq!(paths(&graph), ["crate", "crate::util"]);
    }

    /// A module whose file does not match its name is placed where the
    /// compiler places it; the reading this replaced could not follow it.
    #[test]
    fn a_module_mounted_away_from_its_name_is_still_placed() {
        use rustmanifest_schema::{Resolution, ResolvedModule};

        let sources = [
            source("src/lib.rs", "#[path = \"elsewhere.rs\"]\nmod util;"),
            source("src/elsewhere.rs", "use crate::Item;")
        ];
        let mut settled = Resolution {
            resolver: "test".to_owned(),
            modules: vec![
                ResolvedModule {
                    names:      "demo".to_owned(),
                    file:       PathBuf::from("src/lib.rs"),
                    crate_root: true,
                    parent:     None
                },
                ResolvedModule {
                    names:      "demo::util".to_owned(),
                    file:       PathBuf::from("src/elsewhere.rs"),
                    crate_root: false,
                    parent:     Some("demo".to_owned())
                },
            ],
            ..Resolution::default()
        };
        settled.settle();
        let graph = ModuleGraph::of(&sources, Some(&settled));
        assert_eq!(paths(&graph), ["crate", "crate::util"]);
        let mounted = graph
            .modules()
            .iter()
            .find(|module| module.path == "crate::util")
            .unwrap();
        assert_eq!(
            sources[mounted.source].path(),
            PathBuf::from("src/elsewhere.rs")
        );
    }

    #[test]
    fn the_uses_of_a_file_land_on_the_module_that_file_is() {
        let sources = [source(
            "src/lib.rs",
            "use std::io::{Read, Write};\nuse crate::Item;"
        )];
        let settled = declared_tree(&sources);
        let graph = ModuleGraph::of(&sources, Some(&settled));
        let targets: Vec<&str> = graph.modules()[0]
            .uses
            .iter()
            .map(|held| held.target.as_str())
            .collect();
        assert_eq!(targets, ["std::io::Read", "std::io::Write", "crate::Item"]);
    }

    /// Without a settled tree there is no graph: a guessed mounting reads
    /// exactly like a derived one and is not one.
    #[test]
    fn no_settled_tree_answers_nothing() {
        let sources = [source("src/lib.rs", "mod util;")];
        assert!(ModuleGraph::of(&sources, None).modules().is_empty());
    }
}
