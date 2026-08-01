// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Resolved-name accuracy layer: call edges read from a SCIP index.
//!
//! `rust-analyzer scip .` emits a protobuf index whose occurrences carry
//! name-resolved symbols with definition ranges. This module turns that
//! index into per-caller callee lists the call graph can consume as an
//! optional overlay: a reference occurrence of a function symbol is
//! attributed to the smallest function definition whose enclosing range
//! contains it. The layer is opt-in — the index is produced by an
//! external tool the user runs, never by the engine itself — and purely
//! additive: it can only add edges the syntactic resolver missed, never
//! remove or reclassify one.
//!
//! Documented limits: the index reflects one cfg universe (rust-analyzer
//! indexes active code only, so feature-gated bodies need an index built
//! with those features on), and operator desugarings recorded by
//! rust-analyzer as trait calls (`?`, `.await`, arithmetic) are kept out
//! by the same function-symbol filter that admits real calls.

use std::{collections::BTreeMap, path::Path};

use crate::{semantic::behavior::FunctionFacts, source::Source};

/// Failure to load a SCIP index.
#[derive(Debug, thiserror::Error)]
pub enum ScipError {
    /// The index file could not be read.
    #[error("cannot read SCIP index: {0}")]
    Read(#[from] std::io::Error),
    /// The file's bytes are not a SCIP protobuf index.
    #[error("cannot decode SCIP index: truncated or malformed protobuf")]
    Decode
}

/// Name-resolved call edges of one SCIP index, keyed by caller.
#[derive(Debug, Default)]
pub struct ScipEdges {
    /// `(relative path, caller qualified name)` to callee names.
    by_caller: BTreeMap<(String, String), Vec<String>>
}

impl ScipEdges {
    /// Reads and decodes an index file into caller-keyed call edges.
    ///
    /// # Errors
    ///
    /// Returns [`ScipError::Read`] when the file cannot be read and
    /// [`ScipError::Decode`] when its bytes are not a SCIP index.
    pub fn from_file(path: &Path) -> Result<Self, ScipError> {
        let bytes = std::fs::read(path)?;
        let documents = wire::documents(&bytes).ok_or(ScipError::Decode)?;
        Ok(Self::from_documents(&documents))
    }

    /// Builds caller-keyed edges from decoded documents.
    #[must_use]
    fn from_documents(documents: &[wire::Document]) -> Self {
        let mut by_caller: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
        for document in documents {
            let definitions = function_definitions(document);
            for occurrence in &document.occurrences {
                if occurrence.symbol_roles & DEFINITION_ROLE != 0
                    || !function_symbol(&occurrence.symbol)
                {
                    continue;
                }
                let Some(position) = range_start(&occurrence.range) else {
                    continue;
                };
                let caller = definitions
                    .iter()
                    .filter(|definition| definition.contains(position))
                    .min_by_key(|definition| definition.extent());
                if let Some(definition) = caller {
                    by_caller
                        .entry((document.relative_path.clone(), definition.name.clone()))
                        .or_default()
                        .push(qualified_name(&occurrence.symbol));
                }
            }
        }
        for callees in by_caller.values_mut() {
            callees.sort();
            callees.dedup();
        }
        Self {
            by_caller
        }
    }

    /// Callee names recorded for the function `caller` in the document
    /// whose relative path ends the given source path.
    #[must_use]
    pub fn callees(&self, source_path: &str, caller: &str) -> &[String] {
        self.by_caller
            .iter()
            .find(|((path, name), _)| name == caller && path_matches(source_path, path))
            .map_or(&[], |(_, callees)| callees.as_slice())
    }

    /// Additional call candidates per fact index: SCIP callees of each
    /// function the syntactic candidates may have missed.
    #[must_use]
    pub fn candidates(&self, facts: &[FunctionFacts], sources: &[Source]) -> Vec<Vec<String>> {
        facts
            .iter()
            .map(|fact| {
                sources.get(fact.source).map_or_else(Vec::new, |source| {
                    self.callees(&source.path().to_string_lossy(), &fact.function)
                        .to_vec()
                })
            })
            .collect()
    }
}

/// SCIP symbol-role bit marking a definition occurrence.
const DEFINITION_ROLE: i32 = 1;

/// One function definition with its body extent.
struct Definition {
    /// Qualified name parsed from the symbol.
    name:  String,
    /// Start of the enclosing range as `(line, column)`.
    start: (i32, i32),
    /// End of the enclosing range as `(line, column)`.
    end:   (i32, i32)
}

impl Definition {
    /// Returns whether the position sits inside this definition.
    const fn contains(&self, position: (i32, i32)) -> bool {
        let after_start = self.start.0 < position.0
            || (self.start.0 == position.0 && self.start.1 <= position.1);
        let before_end =
            position.0 < self.end.0 || (position.0 == self.end.0 && position.1 <= self.end.1);
        after_start && before_end
    }

    /// Size of the definition's extent, for smallest-enclosing choice.
    const fn extent(&self) -> (i32, i32) {
        (self.end.0 - self.start.0, self.end.1 - self.start.1)
    }
}

/// Function definitions of one document with their enclosing ranges.
fn function_definitions(document: &wire::Document) -> Vec<Definition> {
    let mut definitions = Vec::new();
    for occurrence in &document.occurrences {
        if occurrence.symbol_roles & DEFINITION_ROLE == 0 || !function_symbol(&occurrence.symbol) {
            continue;
        }
        let range = if occurrence.enclosing_range.is_empty() {
            &occurrence.range
        } else {
            &occurrence.enclosing_range
        };
        let Some((start, end)) = range_bounds(range) else {
            continue;
        };
        definitions.push(Definition {
            name: qualified_name(&occurrence.symbol),
            start,
            end
        });
    }
    definitions
}

/// Returns whether a symbol names a function or method.
fn function_symbol(symbol: &str) -> bool {
    symbol.ends_with("().")
}

/// Start position of a SCIP range, which holds three elements for a
/// single-line range and four for a multi-line one.
fn range_start(range: &[i32]) -> Option<(i32, i32)> {
    match range.len() {
        3 | 4 => Some((range[0], range[1])),
        _ => None
    }
}

/// Start and end positions of a SCIP range.
fn range_bounds(range: &[i32]) -> Option<((i32, i32), (i32, i32))> {
    match range.len() {
        3 => Some(((range[0], range[1]), (range[0], range[2]))),
        4 => Some(((range[0], range[1]), (range[2], range[3]))),
        _ => None
    }
}

/// Renders a SCIP function symbol as the engine's qualified name shape:
/// `Type::method` when the descriptor carries an owner, the bare name
/// otherwise. Descriptors start after the fourth space-separated header
/// token; `impl` owners resolve through the `[Type]` disambiguator, and
/// backtick-escaped generic owners drop their arguments.
fn qualified_name(symbol: &str) -> String {
    let descriptor = symbol
        .splitn(5, ' ')
        .nth(4)
        .unwrap_or(symbol)
        .trim_end_matches("().");
    let segment = descriptor.rsplit('/').next().unwrap_or(descriptor);
    let Some((owner, method)) = segment.split_once('#') else {
        let (_, bare) = split_brackets(segment);
        return strip_generics(&bare);
    };
    let (bracketed, bare) = split_brackets(method);
    let owner = strip_generics(owner);
    let owner = if owner == "impl" || owner.is_empty() {
        bracketed
            .map(|inner| strip_generics(&inner))
            .unwrap_or_default()
    } else {
        owner
    };
    if owner.is_empty() {
        bare
    } else {
        format!("{owner}::{bare}")
    }
}

/// Splits leading `[..]` disambiguators off a descriptor name, keeping
/// the first one — the self type of an `impl` block.
fn split_brackets(name: &str) -> (Option<String>, String) {
    let mut rest = name;
    let mut first = None;
    while rest.starts_with('[') {
        let Some(close) = rest.find(']') else {
            break;
        };
        if first.is_none() {
            first = Some(rest[1..close].to_string());
        }
        rest = &rest[close + 1..];
    }
    (first, rest.to_string())
}

/// Drops backtick escaping and generic arguments from a type name.
fn strip_generics(name: &str) -> String {
    let trimmed = name.trim_matches('`');
    trimmed
        .find('<')
        .map_or_else(|| trimmed.to_string(), |open| trimmed[..open].to_string())
}

/// Returns whether a workspace source path ends with the index's
/// relative document path, seen with forward slashes.
fn path_matches(source_path: &str, relative: &str) -> bool {
    let normalized = source_path.replace('\\', "/");
    let normalized = normalized.trim_start_matches("./");
    normalized == relative || normalized.ends_with(&format!("/{relative}"))
}

/// Minimal protobuf wire-format reader for the SCIP subset the layer
/// consumes: `Index.documents` (field 2), `Document.relative_path`
/// (field 1) and `Document.occurrences` (field 2), and per occurrence
/// `range` (field 1, packed), `symbol` (field 2), `symbol_roles`
/// (field 3), and `enclosing_range` (field 7, packed). Reading the
/// three fields by hand keeps the engine free of a protobuf runtime
/// and its transitive dependency tree; unknown fields are skipped by
/// wire type, so richer indexes stay readable.
mod wire {
    /// One decoded SCIP document.
    pub(super) struct Document {
        /// Path of the document relative to the index root.
        pub(super) relative_path: String,
        /// Every occurrence recorded for the document.
        pub(super) occurrences:   Vec<Occurrence>
    }

    /// One decoded SCIP occurrence.
    #[derive(Default)]
    pub(super) struct Occurrence {
        /// Start and end of the occurrence, three or four elements.
        pub(super) range:           Vec<i32>,
        /// Fully qualified SCIP symbol.
        pub(super) symbol:          String,
        /// Role bitmask; bit 1 marks a definition.
        pub(super) symbol_roles:    i32,
        /// Full extent of the defined item, when the indexer emits it.
        pub(super) enclosing_range: Vec<i32>
    }

    /// Decodes the documents of one SCIP index.
    pub(super) fn documents(bytes: &[u8]) -> Option<Vec<Document>> {
        let mut documents = Vec::new();
        let mut rest = bytes;
        while !rest.is_empty() {
            let (tag, wire_type, after) = tag(rest)?;
            if tag == 2 && wire_type == 2 {
                let (payload, after_payload) = delimited(after)?;
                documents.push(document(payload)?);
                rest = after_payload;
            } else {
                rest = skip(wire_type, after)?;
            }
        }
        Some(documents)
    }

    /// Decodes one `Document` message.
    fn document(bytes: &[u8]) -> Option<Document> {
        let mut relative_path = String::new();
        let mut occurrences = Vec::new();
        let mut rest = bytes;
        while !rest.is_empty() {
            let (tag, wire_type, after) = tag(rest)?;
            match (tag, wire_type) {
                (1, 2) => {
                    let (payload, after_payload) = delimited(after)?;
                    relative_path = String::from_utf8_lossy(payload).into_owned();
                    rest = after_payload;
                }
                (2, 2) => {
                    let (payload, after_payload) = delimited(after)?;
                    occurrences.push(occurrence(payload)?);
                    rest = after_payload;
                }
                _ => rest = skip(wire_type, after)?
            }
        }
        Some(Document {
            relative_path,
            occurrences
        })
    }

    /// Decodes one `Occurrence` message.
    fn occurrence(bytes: &[u8]) -> Option<Occurrence> {
        let mut decoded = Occurrence::default();
        let mut rest = bytes;
        while !rest.is_empty() {
            let (tag, wire_type, after) = tag(rest)?;
            match (tag, wire_type) {
                (1, 2) => {
                    let (payload, after_payload) = delimited(after)?;
                    decoded.range = packed_int32(payload)?;
                    rest = after_payload;
                }
                (1, 0) => {
                    let (value, after_value) = varint(after)?;
                    decoded.range.push(truncate(value));
                    rest = after_value;
                }
                (2, 2) => {
                    let (payload, after_payload) = delimited(after)?;
                    decoded.symbol = String::from_utf8_lossy(payload).into_owned();
                    rest = after_payload;
                }
                (3, 0) => {
                    let (value, after_value) = varint(after)?;
                    decoded.symbol_roles = truncate(value);
                    rest = after_value;
                }
                (7, 2) => {
                    let (payload, after_payload) = delimited(after)?;
                    decoded.enclosing_range = packed_int32(payload)?;
                    rest = after_payload;
                }
                (7, 0) => {
                    let (value, after_value) = varint(after)?;
                    decoded.enclosing_range.push(truncate(value));
                    rest = after_value;
                }
                _ => rest = skip(wire_type, after)?
            }
        }
        Some(decoded)
    }

    /// Reads one field tag: `(field number, wire type, rest)`.
    fn tag(bytes: &[u8]) -> Option<(u64, u8, &[u8])> {
        let (key, rest) = varint(bytes)?;
        let wire_type = u8::try_from(key & 0x7).ok()?;
        Some((key >> 3, wire_type, rest))
    }

    /// Reads one base-128 varint.
    fn varint(bytes: &[u8]) -> Option<(u64, &[u8])> {
        let mut value = 0u64;
        for (index, byte) in bytes.iter().enumerate().take(10) {
            value |= u64::from(byte & 0x7f) << (index * 7);
            if byte & 0x80 == 0 {
                return Some((value, &bytes[index + 1..]));
            }
        }
        None
    }

    /// Reads one length-delimited payload.
    fn delimited(bytes: &[u8]) -> Option<(&[u8], &[u8])> {
        let (length, rest) = varint(bytes)?;
        let length = usize::try_from(length).ok()?;
        (rest.len() >= length).then(|| rest.split_at(length))
    }

    /// Decodes a packed repeated `int32` payload.
    fn packed_int32(bytes: &[u8]) -> Option<Vec<i32>> {
        let mut values = Vec::new();
        let mut rest = bytes;
        while !rest.is_empty() {
            let (value, after) = varint(rest)?;
            values.push(truncate(value));
            rest = after;
        }
        Some(values)
    }

    /// Truncates a decoded varint to the `int32` value it encodes.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_possible_wrap,
        reason = "protobuf int32 semantics: the low 32 bits are the value"
    )]
    const fn truncate(value: u64) -> i32 {
        value as u32 as i32
    }

    /// Skips one field of the given wire type.
    fn skip(wire_type: u8, bytes: &[u8]) -> Option<&[u8]> {
        match wire_type {
            0 => varint(bytes).map(|(_, rest)| rest),
            1 => (bytes.len() >= 8).then(|| &bytes[8..]),
            2 => delimited(bytes).map(|(_, rest)| rest),
            5 => (bytes.len() >= 4).then(|| &bytes[4..]),
            _ => None
        }
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
    use super::{ScipEdges, qualified_name, wire};

    fn put_varint(out: &mut Vec<u8>, mut value: u64) {
        loop {
            let byte = u8::try_from(value & 0x7f).unwrap();
            value >>= 7;
            if value == 0 {
                out.push(byte);
                break;
            }
            out.push(byte | 0x80);
        }
    }

    fn put_field(out: &mut Vec<u8>, field: u64, payload: &[u8]) {
        put_varint(out, (field << 3) | 2);
        put_varint(out, payload.len() as u64);
        out.extend_from_slice(payload);
    }

    fn put_varint_field(out: &mut Vec<u8>, field: u64, value: u64) {
        put_varint(out, field << 3);
        put_varint(out, value);
    }

    fn packed(values: &[i32]) -> Vec<u8> {
        let mut out = Vec::new();
        for value in values {
            put_varint(&mut out, u64::from(u32::try_from(*value).unwrap()));
        }
        out
    }

    fn occurrence(symbol: &str, range: &[i32], roles: i32, enclosing: &[i32]) -> Vec<u8> {
        let mut out = Vec::new();
        put_field(&mut out, 1, &packed(range));
        put_field(&mut out, 2, symbol.as_bytes());
        if roles != 0 {
            put_varint_field(&mut out, 3, u64::from(u32::try_from(roles).unwrap()));
        }
        if !enclosing.is_empty() {
            put_field(&mut out, 7, &packed(enclosing));
        }
        out
    }

    fn index() -> Vec<u8> {
        let mut document = Vec::new();
        put_field(&mut document, 1, b"src/lib.rs");
        put_field(
            &mut document,
            2,
            &occurrence(
                "rust-analyzer cargo demo 0.1.0 lib/caller().",
                &[0, 3, 9],
                super::DEFINITION_ROLE,
                &[0, 0, 4, 1]
            )
        );
        put_field(
            &mut document,
            2,
            &occurrence(
                "rust-analyzer cargo demo 0.1.0 lib/Wallet#credit().",
                &[2, 4, 10],
                0,
                &[]
            )
        );
        put_field(
            &mut document,
            2,
            &occurrence(
                "rust-analyzer cargo std https://github.com/rust-lang/rust/library/std \
                 fs/remove_dir_all().",
                &[3, 4, 18],
                0,
                &[]
            )
        );
        let mut out = Vec::new();
        put_field(&mut out, 2, &document);
        out
    }

    #[test]
    fn references_attribute_to_their_enclosing_definition() {
        let documents = wire::documents(&index()).unwrap();
        let edges = ScipEdges::from_documents(&documents);
        let callees = edges.callees("./src/lib.rs", "caller");
        assert_eq!(callees, ["Wallet::credit", "remove_dir_all"]);
        assert!(edges.callees("./src/lib.rs", "unknown").is_empty());
        assert!(edges.callees("./other.rs", "caller").is_empty());
    }

    #[test]
    fn malformed_bytes_decode_to_nothing() {
        assert!(wire::documents(&[0x12]).is_none());
        assert!(wire::documents(&[]).is_some_and(|documents| documents.is_empty()));
    }

    #[test]
    fn symbols_render_as_engine_qualified_names() {
        assert_eq!(
            qualified_name("rust-analyzer cargo demo 0.1.0 app/Wallet#credit()."),
            "Wallet::credit"
        );
        assert_eq!(
            qualified_name("rust-analyzer cargo demo 0.1.0 error/impl#[Error]other()."),
            "Error::other"
        );
        assert_eq!(
            qualified_name("rust-analyzer cargo demo 0.1.0 lib/`Criterion<M>`#bench()."),
            "Criterion::bench"
        );
        assert_eq!(
            qualified_name("rust-analyzer cargo demo 0.1.0 util/helper()."),
            "helper"
        );
    }
}
