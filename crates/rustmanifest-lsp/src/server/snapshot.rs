// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The workspace the answers are computed over: every Rust source the
//! root holds, with the editor's open buffers standing in for their
//! files.
//!
//! An unsaved buffer is the program the author is asking about, so a
//! buffer always outranks the bytes on disk. The snapshot rebuilds only
//! after a document event, not per request: reading a workspace on
//! every keystroke would charge every hover for work nothing changed.

use std::{collections::BTreeMap, path::PathBuf};

use rustmanifest_engine::{
    FunctionFacts, Source, TypeGraph,
    semantic::{Effect, TraitOracle, TypeShape, shapes}
};

/// The sources and derivations answers are computed over, rebuilt after
/// document events.
///
/// The derivations are part of the snapshot on purpose: a hover that
/// re-derived the workspace per request paid over a second on this very
/// repository, and nothing had changed between two hovers of one
/// reading pause.
pub(super) struct Snapshot {
    /// Workspace root the client named at initialize, where it named one.
    root:  Option<PathBuf>,
    /// The last build, absent before the first request.
    built: Option<Built>,
    /// Whether a document event landed since the last build.
    dirty: bool
}

/// One build of the snapshot: the sources and every derivation over
/// them.
struct Built {
    /// The sources, buffers overlaid.
    sources: Vec<Source>,
    /// The behavioral facts of the sources.
    facts:   Vec<FunctionFacts>,
    /// The resolved call graph over the facts.
    graph:   rustmanifest_engine::CallGraph,
    /// Per-function effects closed over the graph.
    reached: Vec<Vec<Effect>>,
    /// The type graph of the sources: definitions, derives, and impls.
    types:   TypeGraph,
    /// The memory shape of every captured type.
    shapes:  Vec<TypeShape>,
    /// The compiler's trait index, where the workspace carries one
    /// (the newest rustdoc JSON under `target/doc`).
    oracle:  Option<TraitOracle>
}

impl Snapshot {
    /// A snapshot over `root`, empty until the first build.
    pub(super) const fn new(root: Option<PathBuf>) -> Self {
        Self {
            root,
            built: None,
            dirty: true
        }
    }

    /// Marks the snapshot for rebuild.
    pub(super) const fn touch(&mut self) {
        self.dirty = true;
    }

    /// The current sources and derivations, rebuilt where a document
    /// event landed.
    pub(super) fn derived(&mut self, documents: &BTreeMap<String, String>) -> Derived<'_> {
        if self.dirty || self.built.is_none() {
            let sources = build(self.root.as_deref(), documents);
            let facts = rustmanifest_engine::behavior(&sources);
            let graph = rustmanifest_engine::call_graph(
                &facts,
                &rustmanifest_engine::semantic::resolution_context(&sources)
            );
            let reached = rustmanifest_engine::transitive_effects(&facts, &graph);
            let types = TypeGraph::from_sources(&sources);
            let shaped = shapes(&sources);
            let oracle = compiler_index(self.root.as_deref());
            self.built = Some(Built {
                sources,
                facts,
                graph,
                reached,
                types,
                shapes: shaped,
                oracle
            });
            self.dirty = false;
        }
        let held = self.built.get_or_insert_with(|| Built {
            sources: Vec::new(),
            facts:   Vec::new(),
            graph:   rustmanifest_engine::call_graph(
                &[],
                &rustmanifest_engine::semantic::ResolutionContext::default()
            ),
            reached: Vec::new(),
            types:   TypeGraph::from_sources(&[]),
            shapes:  Vec::new(),
            oracle:  None
        });
        Derived {
            sources: &held.sources,
            facts:   &held.facts,
            graph:   &held.graph,
            reached: &held.reached,
            types:   &held.types,
            shapes:  &held.shapes,
            oracle:  held.oracle.as_ref()
        }
    }
}

/// The newest rustdoc JSON emission under the root's `target/doc`, read
/// as the compiler's trait index; nothing where none exists. One file
/// is read — the newest — so a multi-crate workspace answers for the
/// crate documented last, and the boundary stays honest: absent index,
/// absent answers.
fn compiler_index(root: Option<&std::path::Path>) -> Option<TraitOracle> {
    let dir = root?.join("target").join("doc");
    let newest = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "json")
        })
        .max_by_key(|entry| entry.metadata().and_then(|meta| meta.modified()).ok())?;
    TraitOracle::from_rustdoc_json(&std::fs::read_to_string(newest.path()).ok()?)
}

/// One borrowed view of everything a request computes its answer over.
pub(super) struct Derived<'a> {
    /// The sources, buffers overlaid.
    pub(super) sources: &'a [Source],
    /// The behavioral facts of the sources.
    pub(super) facts:   &'a [FunctionFacts],
    /// The resolved call graph over the facts.
    pub(super) graph:   &'a rustmanifest_engine::CallGraph,
    /// Per-function effects closed over the graph.
    pub(super) reached: &'a [Vec<Effect>],
    /// The type graph of the sources: definitions, derives, and impls.
    pub(super) types:   &'a TypeGraph,
    /// The memory shape of every captured type.
    pub(super) shapes:  &'a [TypeShape],
    /// The compiler's trait index, where the workspace carries one.
    pub(super) oracle:  Option<&'a TraitOracle>
}

/// Builds the source slice: the root's files off disk, buffers overlaid,
/// and buffers outside the root appended.
fn build(root: Option<&std::path::Path>, documents: &BTreeMap<String, String>) -> Vec<Source> {
    let mut held: BTreeMap<PathBuf, String> = BTreeMap::new();
    if let Some(root) = root
        && let Ok(files) = rustmanifest_engine::walker::discover(&[root.to_path_buf()])
    {
        for file in files {
            let readable = std::fs::metadata(&file).is_ok_and(|meta| {
                meta.len() <= rustmanifest_engine::orchestrator::DEFAULT_MAX_FILE_BYTES
            });
            if readable && let Ok(text) = std::fs::read_to_string(&file) {
                held.insert(file, text);
            }
        }
    }
    for (uri, text) in documents {
        let path = PathBuf::from(uri.strip_prefix("file://").unwrap_or(uri));
        held.insert(path, text.clone());
    }
    held.into_iter()
        .map(|(path, text)| Source::new(path, text))
        .collect()
}

/// The path one document uri names.
pub(super) fn path_of(uri: &str) -> PathBuf {
    PathBuf::from(uri.strip_prefix("file://").unwrap_or(uri))
}
