// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The written text in words, rendered without standing in a value the
//! analysis cannot see.
//!
//! A payload is rendered exactly as far as it is proven: a literal keeps
//! its text with whatever terminator the construct adds, a template
//! keeps its proven text and leaves the holes as holes, and a payload
//! that could not be read states why instead of showing a value.

use crate::semantic::behavior::{Hole, HoleSource, Newline, OpaqueReason, Payload, Segment};

/// Appends the line terminator a construct adds, so the rendered text
/// is what actually reaches the stream.
pub(super) fn with_terminator(text: &str, newline: Newline) -> String {
    match newline {
        Newline::Appended => format!("{text}\n"),
        Newline::None => text.to_owned()
    }
}

/// Why a payload could not be read, in words.
pub(super) const fn reason_text(reason: OpaqueReason) -> &'static str {
    match reason {
        OpaqueReason::RuntimeValue => "the written value is computed at runtime",
        OpaqueReason::NonLiteralFormat => "the format string is not a literal",
        OpaqueReason::Unparsed => "the arguments did not parse",
        OpaqueReason::DebugMacro => "the debugging macro embeds its own source position"
    }
}

/// Renders a payload without ever claiming an unproven value.
pub(super) fn describe(payload: &Payload, newline: Newline) -> String {
    let terminator = match newline {
        Newline::Appended => " followed by a line terminator",
        Newline::None => ""
    };
    match payload {
        Payload::Empty => "nothing".to_owned(),
        Payload::Literal(text) => format!("the string `{text}`{terminator}"),
        Payload::Template(segments) => {
            let shape: String = segments
                .iter()
                .map(|segment| match segment {
                    Segment::Text(text) => text.clone(),
                    Segment::Hole(hole) => format!("<hole:{}>", hole_name(hole))
                })
                .collect();
            format!(
                "text of the form `{shape}`{terminator}, where each hole is a runtime value this \
                 analysis does not determine"
            )
        }
        Payload::Opaque(reason) => format!(
            "a value this analysis cannot read ({})",
            match reason {
                OpaqueReason::RuntimeValue => "the written value is computed at runtime",
                OpaqueReason::NonLiteralFormat => "the format string is not a literal",
                OpaqueReason::Unparsed => "the arguments did not parse",
                OpaqueReason::DebugMacro => "the debugging macro embeds its own source position"
            }
        )
    }
}

/// Names one hole by whatever the source gives it.
pub(super) fn hole_name(hole: &Hole) -> String {
    match &hole.source {
        HoleSource::Named(name) => name.clone(),
        HoleSource::Positional(index) | HoleSource::Indexed(index) => hole
            .expr
            .clone()
            .unwrap_or_else(|| format!("argument {index}"))
    }
}
