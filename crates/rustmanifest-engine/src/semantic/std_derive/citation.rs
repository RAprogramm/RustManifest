// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a fact cites: the byte range of the proving text and the text
//! itself.
//!
//! A fact never restates the library. It names a range in a file and
//! carries the library's own characters at that range, so the two
//! conversions every derivation needs — a span into the width the schema
//! records, and a range into a one-line quote — live here and nowhere
//! else.

use rustmanifest_schema::Range;

/// The library's own text at a range, with runs of whitespace collapsed
/// so one quote stays one line.
pub(super) fn quoted(text: &str, range: Range) -> String {
    text.get(range.start as usize..range.end as usize)
        .unwrap_or_default()
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
}

/// A span's byte range, clamped to the width the schema records.
pub(super) fn byte_range(span: &std::ops::Range<usize>) -> Range {
    Range {
        start: u32::try_from(span.start).unwrap_or(u32::MAX),
        end:   u32::try_from(span.end).unwrap_or(u32::MAX)
    }
}
