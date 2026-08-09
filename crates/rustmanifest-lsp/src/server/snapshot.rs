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

use rustmanifest_engine::Source;

/// The sources answers are computed over, rebuilt after document events.
pub(super) struct Snapshot {
    /// Workspace root the client named at initialize, where it named one.
    root:    Option<PathBuf>,
    /// The built sources, buffers overlaid.
    sources: Vec<Source>,
    /// Whether a document event landed since the last build.
    dirty:   bool
}

impl Snapshot {
    /// A snapshot over `root`, empty until the first build.
    pub(super) const fn new(root: Option<PathBuf>) -> Self {
        Self {
            root,
            sources: Vec::new(),
            dirty: true
        }
    }

    /// Marks the snapshot for rebuild.
    pub(super) const fn touch(&mut self) {
        self.dirty = true;
    }

    /// The current sources, rebuilt where a document event landed.
    pub(super) fn sources(&mut self, documents: &BTreeMap<String, String>) -> &[Source] {
        if self.dirty {
            self.sources = build(self.root.as_deref(), documents);
            self.dirty = false;
        }
        &self.sources
    }
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
