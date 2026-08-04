// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What the construct itself does with the failure its operation can
//! have.
//!
//! A closed set naming a machinery distinction whose cost and whose panic
//! differ, so a claim about one is never stated of another. Two spellings
//! of the same addition end the program two different ways, and nothing
//! but this field tells them apart.

/// What the construct does with the failure the operation can have.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Guarding {
    /// The construct does not check. What happens on failure is fixed
    /// outside the source this tier reads — by the build's own settings —
    /// so nothing about it is claimed here.
    Unchecked,
    /// The failure comes back as an absent value.
    Checked,
    /// The result clamps at the bound of the produced type.
    Saturating,
    /// The result wraps around the bound of the produced type.
    Wrapping,
    /// The failure comes back as a value beside the result.
    Reported
}

impl std::fmt::Display for Guarding {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(match self {
            Self::Unchecked => "unchecked",
            Self::Checked => "checked",
            Self::Saturating => "saturating",
            Self::Wrapping => "wrapping",
            Self::Reported => "reported"
        })
    }
}
