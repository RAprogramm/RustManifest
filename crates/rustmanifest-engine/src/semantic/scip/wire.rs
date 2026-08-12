// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Reading a SCIP index through the format's own bindings.
//!
//! A hand-written wire-format reader stood here, decoding the four fields
//! this layer consumes and skipping the rest by wire type. It worked for
//! the indexes it had been shown, and every field the format adds is a
//! field it would have to learn by hand. The format ships bindings; they
//! are read here instead.
//!
//! The shapes below stay because the layer above is written against them,
//! and they carry exactly what it reads — a document's path and, per
//! occurrence, its range, its symbol, the roles it plays, and the range of
//! the item enclosing it.

use scip::types::Index;

/// One document of an index.
pub(super) struct Document {
    /// Path of the document relative to the index root.
    pub(super) relative_path: String,
    /// Every occurrence recorded for the document.
    pub(super) occurrences:   Vec<Occurrence>
}

/// One occurrence of a symbol in a document.
#[derive(Default)]
pub(super) struct Occurrence {
    /// Start and end of the occurrence, three or four elements.
    pub(super) range:           Vec<i32>,
    /// Symbol the occurrence names.
    pub(super) symbol:          String,
    /// Bit set of the roles the occurrence plays.
    pub(super) symbol_roles:    i32,
    /// Range of the item enclosing the occurrence, where the index
    /// records one.
    pub(super) enclosing_range: Vec<i32>
}

/// Reads every document of a serialized index, or nothing where the bytes
/// are not one.
pub(super) fn documents(bytes: &[u8]) -> Option<Vec<Document>> {
    let index: Index = protobuf::Message::parse_from_bytes(bytes).ok()?;
    Some(
        index
            .documents
            .into_iter()
            .map(|document| Document {
                relative_path: document.relative_path,
                occurrences:   document
                    .occurrences
                    .into_iter()
                    .map(|held| Occurrence {
                        range:           held.range,
                        symbol:          held.symbol,
                        symbol_roles:    held.symbol_roles,
                        enclosing_range: held.enclosing_range
                    })
                    .collect()
            })
            .collect()
    )
}
