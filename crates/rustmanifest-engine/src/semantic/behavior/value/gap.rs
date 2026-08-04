// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Why an origin was not settled.
//!
//! Every variant names one recognizer question that failed, so a gap is
//! never read as an absence of the thing it failed on: a value whose
//! origin is not derived is not a value that came from nowhere.

/// Why an origin was not settled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OriginGap {
    /// The expression is outside the shapes this reading recognizes.
    Unreadable,
    /// The name is bound or assigned more than once in this body, so
    /// which of them stands here is a question of scopes this tier does
    /// not resolve.
    Rebound,
    /// The value arrives from outside this body — a field, a static, a
    /// captured name — whose origin is decided elsewhere.
    Foreign,
    /// The site writes no argument at this position, so an input of the
    /// callee has nothing to be substituted with.
    Unwritten,
    /// The call candidate names no single workspace function.
    CalleeUnresolved,
    /// The candidate names code outside the workspace and no knowledge
    /// row states what it hands back.
    CalleeExternal,
    /// The callee sits in the caller's own recursion cycle, so its
    /// summary is not settled when the caller's is derived.
    Recursion,
    /// The call is one of several the dispatch admits; crediting the
    /// caller with any one of their origins would state a certainty the
    /// graph has already declared it does not have.
    Dispatched,
    /// The origin continues past the depth this tier unfolds.
    Deeper,
    /// More alternatives were written than this tier enumerates.
    Unenumerated
}

impl OriginGap {
    /// The question this gap names, as a stable key.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Unreadable => "unreadable",
            Self::Rebound => "rebound",
            Self::Foreign => "foreign",
            Self::Unwritten => "unwritten",
            Self::CalleeUnresolved => "callee-unresolved",
            Self::CalleeExternal => "callee-external",
            Self::Recursion => "recursion",
            Self::Dispatched => "dispatched",
            Self::Deeper => "deeper",
            Self::Unenumerated => "unenumerated"
        }
    }
}

impl std::fmt::Display for OriginGap {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(self.label())
    }
}
