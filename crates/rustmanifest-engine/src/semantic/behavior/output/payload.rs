// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a write puts on its stream, and how much of it is proven.
//!
//! The honesty rule lives in the types: proven text and unproven
//! positions are different variants, so a payload whose text is not
//! fully recovered cannot be rendered as a string
//! ([`Payload::fully_known`] refuses). Recovering a payload out of the
//! syntax of a writing construct sits in the submodules beside this
//! file, so the shape of the answer and the reading that produces it
//! stay separable.

mod reading;
mod segmentation;

pub(in crate::semantic::behavior) use self::reading::{
    argument_payload, literal_bytes_payload, macro_payload
};

/// Where a hole's value comes from in the format string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HoleSource {
    /// An implicit positional argument, in order of appearance.
    Positional(u32),
    /// An explicit argument index.
    Indexed(u32),
    /// A named argument or captured identifier.
    Named(String)
}

/// One unproven position inside a format template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hole {
    /// Which argument fills this position.
    pub source: HoleSource,
    /// Format specification as written, without the braces.
    pub spec:   String,
    /// Argument expression as written, kept as provenance only — it is
    /// never evaluated, folded, or propagated.
    pub expr:   Option<String>
}

/// One piece of a format template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    /// Text proven to appear verbatim.
    Text(String),
    /// A position whose value this analysis does not determine.
    Hole(Hole)
}

/// Why a payload could not be recovered as text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpaqueReason {
    /// The written value is a runtime expression, not a literal.
    RuntimeValue,
    /// The format string itself is not a literal.
    NonLiteralFormat,
    /// The construct's arguments did not parse as expressions.
    Unparsed,
    /// A debugging macro whose rendering embeds source position data
    /// this tier does not compute.
    DebugMacro
}

/// What a write puts on the stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Payload {
    /// Nothing is written — a flush or an empty write.
    Empty,
    /// Text proven in full.
    Literal(String),
    /// Proven text interleaved with holes.
    Template(Vec<Segment>),
    /// Nothing about the text is proven, with the reason.
    Opaque(OpaqueReason)
}

impl Payload {
    /// The written text when every part of it is proven, otherwise
    /// `None`.
    ///
    /// A template with a single hole is not fully known however much
    /// proven text surrounds it — the same rule the standard library's
    /// formatting arguments apply to their own string view.
    #[must_use]
    pub fn fully_known(&self) -> Option<&str> {
        match self {
            Self::Empty => Some(""),
            Self::Literal(text) => Some(text),
            Self::Template(_) | Self::Opaque(_) => None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Hole, HoleSource, OpaqueReason, Payload, Segment};

    #[test]
    fn a_payload_is_fully_known_only_when_every_part_is_proven() {
        assert_eq!(Payload::Empty.fully_known(), Some(""));
        assert_eq!(
            Payload::Literal("Hello, world!".to_owned()).fully_known(),
            Some("Hello, world!")
        );
        let template = Payload::Template(vec![
            Segment::Text("count: ".to_owned()),
            Segment::Hole(Hole {
                source: HoleSource::Positional(0),
                spec:   String::new(),
                expr:   Some("total".to_owned())
            }),
        ]);
        assert_eq!(template.fully_known(), None);
        assert_eq!(
            Payload::Opaque(OpaqueReason::RuntimeValue).fully_known(),
            None
        );
    }
}
