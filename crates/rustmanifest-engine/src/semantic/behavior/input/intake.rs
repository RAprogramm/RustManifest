// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! By what machinery a value reaches the program, and what the reading
//! construct does with the line terminator.

/// By what machinery a value reaches the program.
///
/// Not cosmetic. A value the library hands over as text is validated to
/// be text on the way in, and one that fails that check ends the read on
/// the library's own terms rather than reaching the program. A value
/// handed over as the operating system's own bytes skips the check
/// entirely. Any cost or panic following from that validation is a claim
/// about one intake only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Intake {
    /// The library validates the value as text on the way in.
    Text,
    /// The value arrives as the operating system's own bytes.
    Raw
}

impl std::fmt::Display for Intake {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(match self {
            Self::Text => "text",
            Self::Raw => "raw"
        })
    }
}

/// What the reading construct does with the line terminator.
///
/// Three answers against the write channel's two: a write either appends
/// a terminator or does not, while a read may keep one, remove one, or
/// never meet one. That one reading construct keeps it and another strips
/// it is a fact of the library and the ordinary reason a conversion
/// downstream of it fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Terminator {
    /// The terminator stays on the value.
    Retained,
    /// The construct removes it.
    Stripped,
    /// No terminator is met.
    None
}

impl std::fmt::Display for Terminator {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(match self {
            Self::Retained => "retained",
            Self::Stripped => "stripped",
            Self::None => "none"
        })
    }
}

/// What shape the site takes off the channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReadShape {
    /// Everything the channel yields, one after another.
    Sequence,
    /// One element of it.
    Element,
    /// One run of text.
    Text
}

impl std::fmt::Display for ReadShape {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(match self {
            Self::Sequence => "sequence",
            Self::Element => "element",
            Self::Text => "text"
        })
    }
}
