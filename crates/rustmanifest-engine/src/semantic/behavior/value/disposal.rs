// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What the source does with a failure a site can produce.

/// What the source does with a failure a site can produce.
///
/// The question the conduit reading asks of a write and nothing asks of a
/// fallible value today: the same call disposed three ways ends the
/// program three ways, so a claim about one is never stated of another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Disposal {
    /// Handed back to the caller by `?`.
    Propagated,
    /// Turned into a panic by the unwrapping pair.
    Panicked,
    /// Replaced by a value the site writes.
    Defaulted,
    /// Taken apart by a choice at the site.
    Examined,
    /// The site produces no failure, so nothing disposes of one.
    Infallible,
    /// A failure can arise here and what becomes of it is not identified.
    /// Never the reading for a call merely absent from the knowledge
    /// rows: absence of a row is absence of knowledge, not proof that the
    /// call cannot fail.
    NotIdentified
}

impl Disposal {
    /// The disposal as a stable key.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Propagated => "propagated",
            Self::Panicked => "panicked",
            Self::Defaulted => "defaulted",
            Self::Examined => "examined",
            Self::Infallible => "infallible",
            Self::NotIdentified => "not-identified"
        }
    }
}

impl std::fmt::Display for Disposal {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(self.label())
    }
}
