// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The corpus resource surface: the engine's own measurement, exposed
//! through its own API.
//!
//! A system whose claims rest on being measured owes its measurement to
//! every consumer. Where the workspace root carries the evaluation bed
//! (`eval/corpus.toml`, the blessed `eval/baseline.json`, and the last
//! `eval/report.json`), each file is served verbatim as a resource
//! under `rustmanifest://corpus/` — the pinned repositories, the
//! blessed per-rule counts a rerun must match, and the last recorded
//! run with its timings. A workspace without the bed lists no corpus
//! resources: absent measurement, absent claims.

use std::path::Path;

use rmcp::{
    ErrorData,
    model::{ReadResourceResponse, ReadResourceResult, Resource, ResourceContents}
};

/// The corpus files served, with their URIs and meanings.
const SERVED: [(&str, &str, &str, &str, &str); 3] = [
    (
        "rustmanifest://corpus/manifest",
        "corpus-manifest",
        "Evaluation corpus",
        "The pinned repositories the engine is measured against.",
        "eval/corpus.toml"
    ),
    (
        "rustmanifest://corpus/baseline",
        "corpus-baseline",
        "Blessed baseline",
        "Per-rule finding counts a corpus rerun must match exactly.",
        "eval/baseline.json"
    ),
    (
        "rustmanifest://corpus/report",
        "corpus-report",
        "Last corpus report",
        "The last recorded corpus run, timings included.",
        "eval/report.json"
    )
];

/// Lists every corpus file the workspace root carries.
pub(super) fn listing(root: &Path) -> Vec<Resource> {
    SERVED
        .iter()
        .filter(|(_, _, _, _, path)| root.join(path).is_file())
        .map(|(uri, name, title, description, path)| {
            Resource::new(*uri, *name)
                .with_title(*title)
                .with_description(format!("{description} Served from `{path}`."))
                .with_mime_type(mime_of(path))
        })
        .collect()
}

/// Reads the corpus file a requested URI resolves to, or nothing when
/// the URI is not a corpus one.
pub(super) fn read(root: &Path, uri: &str) -> Option<Result<ReadResourceResponse, ErrorData>> {
    let (_, _, _, _, path) = SERVED.iter().find(|(served, ..)| *served == uri)?;
    Some(match std::fs::read_to_string(root.join(path)) {
        Ok(content) => Ok(ReadResourceResult::new(vec![
            ResourceContents::text(content, uri).with_mime_type(mime_of(path)),
        ])
        .into()),
        Err(error) => Err(ErrorData::resource_not_found(
            format!("corpus file `{path}` is not readable: {error}"),
            None
        ))
    })
}

/// The mime type a corpus file serves as.
fn mime_of(path: &str) -> &'static str {
    if Path::new(path)
        .extension()
        .is_some_and(|extension| extension == "json")
    {
        "application/json"
    } else {
        "application/toml"
    }
}
