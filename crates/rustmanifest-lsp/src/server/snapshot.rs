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
    semantic::{Effect, TraitOracle, TypeShape, shapes_with}
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
    /// The compiler's trait index, where the workspace carries one.
    oracle:  Option<Indexed>,
    /// What a resolver settled about the sources, where the workspace
    /// carries an overlay beside its manifest.
    settled: Option<rustmanifest_schema::Resolution>
}

/// The compiler's trait index with its freshness reading.
pub(super) struct Indexed {
    /// Every documented crate's emissions merged into one index.
    pub(super) oracle: TraitOracle,
    /// Whether some source is younger than the newest emission — the
    /// index then speaks with its age named, degraded by words rather
    /// than silenced: its knowledge does not become false wholesale
    /// because one file is newer.
    pub(super) stale:  bool
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
            let settled = settled_overlay(self.root.as_deref());
            let facts = rustmanifest_engine::behavior_with(&sources, settled.as_ref());
            let graph = rustmanifest_engine::call_graph(
                &facts,
                &rustmanifest_engine::semantic::resolution_context_with(
                    &sources,
                    settled.as_ref()
                )
            );
            let reached = rustmanifest_engine::transitive_effects(&facts, &graph);
            let types = TypeGraph::from_sources(&sources);
            let shaped = shapes_with(&sources, settled.as_ref());
            let oracle = compiler_index(self.root.as_deref(), &sources);
            self.built = Some(Built {
                sources,
                facts,
                graph,
                reached,
                types,
                shapes: shaped,
                oracle,
                settled
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
            oracle:  None,
            settled: None
        });
        Derived {
            sources: &held.sources,
            facts:   &held.facts,
            graph:   &held.graph,
            reached: &held.reached,
            types:   &held.types,
            shapes:  &held.shapes,
            oracle:  held.oracle.as_ref(),
            settled: held.settled.as_ref()
        }
    }
}

/// What a resolver settled about this workspace, read from
/// `resolution.json` beside the manifest; nothing where none was written.
///
/// The same discipline as the documentation index: absent overlay, absent
/// answers. What it carries — the type of every expression and whether the
/// compiler copies or moves each type — is what no reading of the written
/// text can state, so its absence is a quieter editor and never a guess.
fn settled_overlay(root: Option<&std::path::Path>) -> Option<rustmanifest_schema::Resolution> {
    let root = root?;
    let text = std::fs::read_to_string(root.join("resolution.json")).ok()?;
    let mut settled: rustmanifest_schema::Resolution = serde_json::from_str(&text).ok()?;
    rebase(root, &mut settled);
    Some(settled)
}

/// Rewrites every path the overlay states onto `root`.
///
/// The overlay names files from the workspace root, as its shape promises;
/// the server holds the same files by the absolute path it read them from.
/// A lookup is by equality, so without this the overlay matches nothing
/// and reads exactly like a resolver that settled nothing.
fn rebase(root: &std::path::Path, settled: &mut rustmanifest_schema::Resolution) {
    for call in &mut settled.calls {
        call.at.file = root.join(&call.at.file);
    }
    for field in &mut settled.fields {
        field.at.file = root.join(&field.at.file);
    }
    for typed in &mut settled.types {
        typed.at.file = root.join(&typed.at.file);
    }
    for path in &mut settled.paths {
        path.at.file = root.join(&path.at.file);
    }
    for module in &mut settled.modules {
        module.file = root.join(&module.file);
    }
    settled.settle();
}

/// Every rustdoc JSON emission under the root's `target/doc`, merged
/// into one compiler trait index; nothing where none exists. Reading
/// them all lets a multi-crate workspace answer for every documented
/// crate rather than the one documented last, and the boundary stays
/// honest: absent index, absent answers. The freshness reading
/// compares the newest emission against the newest source on disk.
fn compiler_index(root: Option<&std::path::Path>, sources: &[Source]) -> Option<Indexed> {
    let dir = root?.join("target").join("doc");
    let mut oracle: Option<TraitOracle> = None;
    let mut newest_emission: Option<std::time::SystemTime> = None;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|extension| extension != "json") {
            continue;
        }
        let Some(read) = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| TraitOracle::from_rustdoc_json(&text))
        else {
            continue;
        };
        oracle = Some(match oracle {
            Some(held) => held.merged(read),
            None => read
        });
        if let Ok(written) = entry.metadata().and_then(|meta| meta.modified()) {
            newest_emission = Some(newest_emission.map_or(written, |held| held.max(written)));
        }
    }
    let newest_source = sources
        .iter()
        .filter_map(|source| {
            std::fs::metadata(source.path())
                .and_then(|meta| meta.modified())
                .ok()
        })
        .max();
    Some(Indexed {
        oracle: oracle?,
        stale:  match (newest_emission, newest_source) {
            (Some(emitted), Some(written)) => written > emitted,
            _ => false
        }
    })
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
    /// The compiler's trait index with its freshness reading, where the
    /// workspace carries one.
    pub(super) oracle:  Option<&'a Indexed>,
    /// What a resolver settled about the sources, where the workspace
    /// carries an overlay.
    pub(super) settled: Option<&'a rustmanifest_schema::Resolution>
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
