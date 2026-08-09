// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Byte offsets to protocol positions.
//!
//! The engine records half-open byte ranges; the protocol speaks in
//! lines and UTF-16 code units. The index is built once per document
//! text and answers per offset, clamping past-the-end offsets to the
//! end of the text rather than inventing a position the text does not
//! have.

use lsp_types::Position;

/// Line starts of one document text, as byte offsets.
pub(super) struct LineIndex {
    starts: Vec<usize>
}

impl LineIndex {
    /// Builds the index of `text`.
    pub(super) fn new(text: &str) -> Self {
        let mut starts = vec![0];
        starts.extend(
            text.char_indices()
                .filter(|&(_, part)| part == '\n')
                .map(|(at, _)| at + 1)
        );
        Self {
            starts
        }
    }

    /// The protocol position of one byte offset in `text`.
    pub(super) fn position(&self, text: &str, offset: usize) -> Position {
        let clamped = offset.min(text.len());
        let line = self
            .starts
            .partition_point(|&start| start <= clamped)
            .saturating_sub(1);
        let from = self.starts.get(line).copied().unwrap_or(0);
        let column: usize = text[from..clamped].chars().map(char::len_utf16).sum();
        Position {
            line:      u32::try_from(line).unwrap_or(u32::MAX),
            character: u32::try_from(column).unwrap_or(u32::MAX)
        }
    }
}

/// The byte offset one protocol position names in `text`, UTF-16 code
/// units walked back to bytes, clamped to the line it names.
pub(super) fn offset_of(text: &str, position: lsp_types::Position) -> usize {
    let mut line = 0u32;
    let mut cursor = 0usize;
    if position.line > 0 {
        for (at, part) in text.char_indices() {
            if part == '\n' {
                line += 1;
                if line == position.line {
                    cursor = at + 1;
                    break;
                }
            }
        }
        if line < position.line {
            return text.len();
        }
    }
    let mut units = 0u32;
    for (at, part) in text[cursor..].char_indices() {
        if units >= position.character || part == '\n' {
            return cursor + at;
        }
        units += u32::try_from(part.len_utf16()).unwrap_or(u32::MAX);
    }
    text.len()
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use super::LineIndex;

    #[test]
    fn offsets_map_to_lines_and_utf16_columns() {
        let text = "fn a() {}\nlet s = \"п\";\n";
        let index = LineIndex::new(text);
        assert_eq!(index.position(text, 0).line, 0);
        assert_eq!(index.position(text, 10).line, 1);
        assert_eq!(index.position(text, 10).character, 0);
        let past = index.position(text, 10_000);
        assert_eq!(past.line, 2);
    }
}
