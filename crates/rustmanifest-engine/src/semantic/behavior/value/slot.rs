// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! One declared input of the function an origin is written in terms of.

/// One declared input of the function an origin is written in terms of.
///
/// The position counts every input the signature declares, the receiver
/// first, so a call site's written arguments align with it. It is
/// deliberately not a position in [`FunctionFacts::params`]: that list
/// drops an input it cannot profile — a receiver with no impl type, a
/// pattern that is not a plain name, a type whose last segment does not
/// render — and its positions therefore align with nothing.
///
/// [`FunctionFacts::params`]: crate::semantic::FunctionFacts::params
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ParamSlot {
    /// Position among the declared inputs, the receiver first.
    pub position: u8,
    /// Name as written, or `None` where the input is a pattern this tier
    /// does not take apart.
    pub name:     Option<String>
}
