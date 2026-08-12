// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Byte offsets to protocol positions.
//!
//! The engine records half-open byte ranges; the protocol speaks in lines
//! and UTF-16 code units. Both conversions are the language server's own
//! index, taken from the crate that ships it: it builds line starts once
//! and answers each direction by bisection, where the reading that stood
//! here walked the text character by character on every request.
//!
//! Past-the-end offsets clamp to the end of the text rather than inventing
//! a position the text does not have.

use line_index::{TextSize, WideEncoding};
use lsp_types::Position;

/// Line starts of one document text.
pub(super) struct LineIndex {
    held: line_index::LineIndex
}

impl LineIndex {
    /// Builds the index of `text`.
    pub(super) fn new(text: &str) -> Self {
        Self {
            held: line_index::LineIndex::new(text)
        }
    }

    /// The protocol position of one byte offset in `text`.
    pub(super) fn position(&self, text: &str, offset: usize) -> Position {
        let clamped = u32::try_from(offset.min(text.len())).unwrap_or(u32::MAX);
        let at = self.held.line_col(TextSize::from(clamped));
        let wide = self.held.to_wide(WideEncoding::Utf16, at);
        Position {
            line:      at.line,
            character: wide.map_or(at.col, |held| held.col)
        }
    }

    /// The byte offset one protocol position names, clamped to the end of
    /// the text where it names one past it.
    pub(super) fn offset(&self, text: &str, position: Position) -> usize {
        let wide = line_index::WideLineCol {
            line: position.line,
            col:  position.character
        };
        self.held
            .to_utf8(WideEncoding::Utf16, wide)
            .and_then(|at| self.held.offset(at))
            .map_or(text.len(), |held| usize::from(held).min(text.len()))
    }
}

/// The byte offset one protocol position names in `text`.
pub(super) fn offset_of(text: &str, position: Position) -> usize {
    LineIndex::new(text).offset(text, position)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use super::{LineIndex, offset_of};

    #[test]
    fn offsets_map_to_lines_and_utf16_columns() {
        let text = "fn a() {}\nlet s = \"п\";\n";
        let index = LineIndex::new(text);
        let start = index.position(text, 0);
        assert_eq!((start.line, start.character), (0, 0));
        let second = index.position(text, 10);
        assert_eq!((second.line, second.character), (1, 0));
        let after = index.position(text, text.len() + 99);
        assert_eq!(after.line, 2);
    }

    #[test]
    fn a_position_walks_back_to_the_byte_it_names() {
        let text = "fn a() {}\nlet s = \"п\";\n";
        for offset in [0usize, 3, 10, 19] {
            let position = LineIndex::new(text).position(text, offset);
            assert_eq!(offset_of(text, position), offset, "at {offset}");
        }
    }

    #[test]
    fn a_position_past_the_end_names_the_end() {
        let text = "fn a() {}\n";
        let past = lsp_types::Position {
            line:      99,
            character: 0
        };
        assert_eq!(offset_of(text, past), text.len());
    }
}
