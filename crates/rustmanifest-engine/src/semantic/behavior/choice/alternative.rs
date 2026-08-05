// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! One alternative of a choosing construct.

use super::super::value::Provenance;

/// One alternative of a choosing construct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alternative {
    /// The pattern exactly as written, kept as provenance only — it is
    /// never matched, folded, or propagated.
    pub pattern:     Option<String>,
    /// Whether the pattern accepts every remaining value.
    pub irrefutable: bool,
    /// Whether the alternative carries a condition of its own beyond its
    /// pattern.
    pub conditional: bool,
    /// Where the value it produces comes from. An alternative that leaves
    /// the function produces none, and says so by the question it failed
    /// on rather than by an absence.
    pub value:       Provenance
}

/// How the alternative set of a choice is closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Closure {
    /// An alternative accepts every remaining value and carries no
    /// condition of its own, so exactly one alternative runs.
    CatchAll,
    /// Every variant of a standard type is named, so exactly one
    /// alternative runs.
    ///
    /// A weaker proof than a catch-all and kept apart from it: it rests on
    /// the written constructor names, not on the scrutinee's type, which
    /// this tier does not read. A program spelling its own type's variants
    /// the same way would be credited by it.
    StandardVariants,
    /// No alternative accepts every remaining value, and the type of the
    /// value being chosen on is not read here, so the set is recorded as
    /// it stands rather than claimed closed.
    ///
    /// A choice with one alternative per variant of a type and no
    /// catch-all lands here. Nothing in this fact base enumerates a
    /// type's variants, and this tier does not pretend to — which means
    /// it is less certain about the better-written form, and says so
    /// rather than guessing.
    Unproven
}

impl std::fmt::Display for Closure {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(match self {
            Self::CatchAll => "closed by a catch-all",
            Self::StandardVariants => "closed by naming every variant of a standard type",
            Self::Unproven => "not proven closed"
        })
    }
}
