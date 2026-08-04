// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! By what machinery a payload reaches the stream.
//!
//! The distinction is not cosmetic. A payload handed to the formatting
//! machinery is walked by a formatter, arrives as an argument pack built
//! on the stack, and reaches the stream through a call whose failure the
//! print family turns into a panic. Bytes handed to the write method of
//! the write trait skip all three: no formatter runs, no argument pack is
//! built, and the failure comes back as a value the caller must handle.
//!
//! Any form whose cost or whose panic follows from the formatting
//! machinery is therefore a claim about one conduit only, and must not be
//! stated of a write that never entered it.

/// By what machinery a payload reaches the stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Conduit {
    /// Through the formatting machinery: a format string and an argument
    /// pack walked by a formatter.
    Formatting,
    /// As bytes, straight to the write method of the write trait.
    Bytes
}

impl Conduit {
    /// The word this conduit is named by in a rendered claim.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Formatting => "formatting",
            Self::Bytes => "bytes"
        }
    }
}

impl std::fmt::Display for Conduit {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.label())
    }
}
