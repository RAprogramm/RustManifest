// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Which part of a channel a read draws.

/// Why which part of a channel a site draws is not proven.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SelectorGap {
    /// The part is chosen by a value computed at run time.
    RuntimeValue,
    /// The key is not written as a literal.
    NonLiteralKey,
    /// The part is taken through an adapter that does not keep positions,
    /// so which part a given pass draws is not fixed by the source.
    Advanced
}

impl std::fmt::Display for SelectorGap {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(match self {
            Self::RuntimeValue => "runtime-value",
            Self::NonLiteralKey => "non-literal-key",
            Self::Advanced => "advanced"
        })
    }
}

/// Which part of a channel a read draws.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selector {
    /// The whole of what the channel yields, nothing singled out.
    Whole,
    /// The element at a position written as a number.
    Positional(u32),
    /// The entry under a name written as a text literal.
    Named(String),
    /// Which part is drawn is not proven, with the reason.
    NotIdentified(SelectorGap)
}

impl std::fmt::Display for Selector {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Whole => out.write_str("whole"),
            Self::Positional(position) => write!(out, "position {position}"),
            Self::Named(key) => write!(out, "entry {key:?}"),
            Self::NotIdentified(gap) => write!(out, "not identified ({gap})")
        }
    }
}
