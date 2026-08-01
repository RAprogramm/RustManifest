// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Layer inference: the project's module stratification derived from the
//! resolved call graph.
//!
//! A module depends on another when any of its functions calls into it.
//! Depth is the longest acyclic dependency chain below a module: depth 0
//! modules call no other workspace module — the foundation; higher depths
//! sit further up the architecture. Mutually calling modules cannot be
//! stratified: they are reported as entangled, carry no depth, and
//! contribute nothing to their dependents' depth — honesty over tidiness.

use std::collections::{BTreeMap, BTreeSet};

use super::{behavior::behavior, calls::call_graph, module_overview::module_overview};
use crate::source::Source;

/// One module's inferred position in the architecture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleLayer {
    /// Module path, `crate` for a crate root.
    pub module:     String,
    /// Longest dependency chain below the module; `None` when the module
    /// participates in a mutual-call entanglement.
    pub depth:      Option<u32>,
    /// Workspace modules this module calls into, sorted.
    pub calls_into: Vec<String>
}

/// Infers the layer of every module that owns at least one function.
#[must_use]
pub fn layers(sources: &[Source]) -> Vec<ModuleLayer> {
    let facts = behavior(sources);
    let overview = module_overview(sources);
    let graph = call_graph(&facts, &super::calls::resolution_context(sources));
    let module_of: Vec<String> = facts
        .iter()
        .map(|fact| {
            sources
                .get(fact.source)
                .and_then(|entry| {
                    overview
                        .modules()
                        .iter()
                        .find(|module| module.file == entry.path())
                })
                .map_or_else(|| "?".to_owned(), |module| module.path.clone())
        })
        .collect();
    let mut edges: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (caller, module) in module_of.iter().enumerate() {
        edges.entry(module.as_str()).or_default();
        for &callee in graph.callees(caller) {
            let target = module_of[callee].as_str();
            if target != module && target != "?" {
                edges.entry(module.as_str()).or_default().insert(target);
            }
        }
    }
    let entangled: BTreeSet<&str> = entangled_modules(&edges);
    let mut depths: BTreeMap<&str, u32> = BTreeMap::new();
    let module_count = edges.len();
    for round in 0..=module_count {
        let mut changed = false;
        for (&module, targets) in &edges {
            if entangled.contains(module) {
                continue;
            }
            let depth = targets
                .iter()
                .filter(|target| !entangled.contains(*target))
                .map(|target| depths.get(target).copied().map_or(1, |d| d + 1))
                .max()
                .unwrap_or(0);
            if depths.get(module) != Some(&depth) {
                depths.insert(module, depth);
                changed = true;
            }
        }
        if !changed || round == module_count {
            break;
        }
    }
    edges
        .iter()
        .filter(|(module, _)| **module != "?")
        .map(|(&module, targets)| ModuleLayer {
            module:     module.to_owned(),
            depth:      if entangled.contains(module) {
                None
            } else {
                depths.get(module).copied()
            },
            calls_into: targets.iter().map(|&target| target.to_owned()).collect()
        })
        .collect()
}

/// Modules on any mutual-call path: reachable from themselves.
fn entangled_modules<'graph>(
    edges: &BTreeMap<&'graph str, BTreeSet<&'graph str>>
) -> BTreeSet<&'graph str> {
    let mut entangled = BTreeSet::new();
    for &start in edges.keys() {
        let mut stack: Vec<&str> = edges
            .get(start)
            .map(|targets| targets.iter().copied().collect())
            .unwrap_or_default();
        let mut seen = BTreeSet::new();
        while let Some(current) = stack.pop() {
            if current == start {
                entangled.insert(start);
                break;
            }
            if seen.insert(current)
                && let Some(next) = edges.get(current)
            {
                stack.extend(next.iter().copied());
            }
        }
    }
    entangled
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

    use super::layers;
    use crate::source::Source;

    fn source(path: &str, text: &str) -> Source {
        Source::new(PathBuf::from(path), text.to_owned())
    }

    fn depth_of(all: &[super::ModuleLayer], module: &str) -> Option<u32> {
        all.iter()
            .find(|layer| layer.module == module)
            .unwrap()
            .depth
    }

    #[test]
    fn chains_stratify_by_longest_path() {
        let sources = [
            source("src/lib.rs", "mod api; mod domain; mod storage;"),
            source("src/api.rs", "pub fn handle() { crate::domain::decide(); }"),
            source(
                "src/domain.rs",
                "pub fn decide() { crate::storage::save(); }"
            ),
            source("src/storage.rs", "pub fn save() {}")
        ];
        let all = layers(&sources);
        assert_eq!(depth_of(&all, "crate::storage"), Some(0));
        assert_eq!(depth_of(&all, "crate::domain"), Some(1));
        assert_eq!(depth_of(&all, "crate::api"), Some(2));
    }

    #[test]
    fn mutual_callers_are_entangled_not_ranked() {
        let sources = [
            source("src/lib.rs", "mod a; mod b;"),
            source("src/a.rs", "pub fn ping() { crate::b::pong(); }"),
            source("src/b.rs", "pub fn pong() { crate::a::ping(); }")
        ];
        let all = layers(&sources);
        assert_eq!(depth_of(&all, "crate::a"), None);
        assert_eq!(depth_of(&all, "crate::b"), None);
    }

    #[test]
    fn layers_are_deterministic() {
        let sources = [
            source("src/lib.rs", "mod api; mod storage;"),
            source("src/api.rs", "pub fn handle() { crate::storage::save(); }"),
            source("src/storage.rs", "pub fn save() {}")
        ];
        assert_eq!(layers(&sources), layers(&sources));
    }
}
