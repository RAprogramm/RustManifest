// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Minimal protobuf wire-format reader for the SCIP subset the layer
//! consumes: `Index.documents` (field 2), `Document.relative_path`
//! (field 1) and `Document.occurrences` (field 2), and per occurrence
//! `range` (field 1, packed), `symbol` (field 2), `symbol_roles`
//! (field 3), and `enclosing_range` (field 7, packed). Reading the
//! three fields by hand keeps the engine free of a protobuf runtime
//! and its transitive dependency tree; unknown fields are skipped by
//! wire type, so richer indexes stay readable.

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

#[cfg(test)]
mod tests {
    use super::documents;

    #[test]
    fn malformed_bytes_decode_to_nothing() {
        assert!(documents(&[0x12]).is_none());
        assert!(documents(&[]).is_some_and(|documents| documents.is_empty()));
    }
}
