// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Cutting a format string into the two things it is made of: text
//! proven to appear verbatim, and positions whose value only the run
//! decides.
//!
//! A placeholder that never closes ends the reading rather than a guess
//! about where it meant to stop.

use super::{Hole, HoleSource, OpaqueReason, Payload, Segment};

/// Splits a format string into proven text and holes.
///
/// Escaped braces contribute their single character to the proven text;
/// a placeholder contributes a hole naming its argument. A placeholder
/// that never closes leaves the rest of the string unproven rather than
/// guessing where it ended.
pub(super) fn template(text: &str, arguments: &[String]) -> Payload {
    let mut segments: Vec<Segment> = Vec::new();
    let mut proven = String::new();
    let mut positional = 0u32;
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '{' if characters.peek() == Some(&'{') => {
                characters.next();
                proven.push('{');
            }
            '}' if characters.peek() == Some(&'}') => {
                characters.next();
                proven.push('}');
            }
            '{' => {
                let mut body = String::new();
                let mut closed = false;
                for inner in characters.by_ref() {
                    if inner == '}' {
                        closed = true;
                        break;
                    }
                    body.push(inner);
                }
                if !closed {
                    return Payload::Opaque(OpaqueReason::Unparsed);
                }
                if !proven.is_empty() {
                    segments.push(Segment::Text(std::mem::take(&mut proven)));
                }
                segments.push(Segment::Hole(hole(&body, arguments, &mut positional)));
            }
            _ => proven.push(character)
        }
    }
    if segments.is_empty() {
        return Payload::Literal(proven);
    }
    if !proven.is_empty() {
        segments.push(Segment::Text(proven));
    }
    Payload::Template(segments)
}

/// Builds one hole from a placeholder body, consuming an implicit
/// positional argument when the placeholder names none.
fn hole(body: &str, arguments: &[String], positional: &mut u32) -> Hole {
    let (name, spec) = body.split_once(':').map_or((body, ""), |(n, s)| (n, s));
    let source = if name.is_empty() {
        let index = *positional;
        *positional += 1;
        HoleSource::Positional(index)
    } else if let Ok(index) = name.parse::<u32>() {
        HoleSource::Indexed(index)
    } else {
        HoleSource::Named(name.to_owned())
    };
    let expr = match &source {
        HoleSource::Positional(index) | HoleSource::Indexed(index) => {
            arguments.get(*index as usize).cloned()
        }
        HoleSource::Named(name) => arguments
            .iter()
            .find_map(|argument| argument.strip_prefix(&format!("{name} = ")))
            .map(ToOwned::to_owned)
            .or_else(|| Some(name.clone()))
    };
    Hole {
        source,
        spec: spec.to_owned(),
        expr,
        type_name: None,
        origin: crate::semantic::behavior::value::Provenance::NotDerived(
            crate::semantic::behavior::value::OriginGap::Unreadable
        ),
        alternatives: Vec::new()
    }
}
