// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a value of a known type puts on a stream when a format position
//! renders it.
//!
//! A position filled by a value is unknown only until the value's own
//! display implementation is read. That implementation writes to the
//! formatter it is handed, and every text it can write is already
//! recorded as a write of its own — one per arm of whatever it matches
//! on. Listing them turns "some text goes here" into "one of these texts
//! goes here", which is what the program actually does.
//!
//! Each entry is carried over exactly as it was proven — a text where
//! the implementation writes a text, a shape with positions of its own
//! where it writes one, a stated reason where it could not be read. The
//! list says which writes the implementation records and no more; it does
//! not promise that each entry is fully known, and an entry that is not
//! says so in its own terms.
//!
//! The list is one level deep by construction: it is taken from the facts
//! as they stood before any position was answered, so an entry's own
//! positions are never answered again.

use std::collections::BTreeMap;

use super::{
    output::{Payload, Segment},
    types::FunctionFacts
};

/// The method name a display implementation writes through.
const DISPLAY_METHOD: &str = "fmt";

/// Fills every typed position with the texts its own type can render.
pub(super) fn name_rendered_texts(facts: &mut [FunctionFacts]) {
    let rendered = rendered_texts(facts);
    if rendered.is_empty() {
        return;
    }
    for fact in facts.iter_mut() {
        for write in &mut fact.output_writes {
            let Payload::Template(segments) = &mut write.payload else {
                continue;
            };
            for segment in segments.iter_mut() {
                let Segment::Hole(hole) = segment else {
                    continue;
                };
                let Some(type_name) = &hole.type_name else {
                    continue;
                };
                if let Some(texts) = rendered.get(type_name) {
                    hole.alternatives.clone_from(texts);
                }
            }
        }
    }
}

/// Every type whose display implementation records a write, with what
/// each of those writes puts on the formatter, in the order recorded.
fn rendered_texts(facts: &[FunctionFacts]) -> BTreeMap<String, Vec<Payload>> {
    let mut rendered = BTreeMap::new();
    for fact in facts {
        let Some(owner) = displayed_type(&fact.function) else {
            continue;
        };
        if fact.output_writes.is_empty() {
            continue;
        }
        let written: Vec<Payload> = fact
            .output_writes
            .iter()
            .map(|write| write.payload.clone())
            .collect();
        rendered.insert(owner.to_owned(), written);
    }
    rendered
}

/// The type a recorded function displays, where the function is the
/// display method of an implementation on that type.
fn displayed_type(function: &str) -> Option<&str> {
    let (owner, method) = function.rsplit_once("::")?;
    (method == DISPLAY_METHOD).then_some(owner)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use std::path::PathBuf;

    use super::super::{Payload, Segment, behavior_with};
    use crate::{semantic::settling::Settling, source::Source};

    /// The error type of a small program, its display implementation, and
    /// a `main` printing the error a call handed back.
    const PROGRAM: &str = "use std::fmt;\nenum Failure { Missing(String), Empty }\nimpl \
                           fmt::Display for Failure {\n    fn fmt(&self, out: &mut \
                           fmt::Formatter<'_>) -> fmt::Result {\n        match self {\n            \
                           Self::Missing(name) => write!(out, \"missing: {name}\"),\n            \
                           Self::Empty => out.write_str(\"empty\")\n        }\n    }\n}\nfn \
                           run() -> Result<u8, Failure> { Ok(0) }\nfn main() {\n    match run() \
                           {\n        Ok(value) => println!(\"{value}\"),\n        Err(error) => \
                           eprintln!(\"error: {error}\")\n    }\n}";

    fn alternatives(text: &str, function: &str, position: &str) -> Vec<Payload> {
        settled_alternatives(text, function, position, |settling| settling)
    }

    /// The alternatives listed for one position, read with what `settle`
    /// states a resolver settled about the fixture.
    ///
    /// The name an arm binds is written without a type — which type it
    /// holds is the compiler's answer, and the display implementation
    /// answering the position is found by that type and by nothing else.
    fn settled_alternatives<'text>(
        text: &'text str,
        function: &str,
        position: &str,
        settle: impl FnOnce(Settling<'text>) -> Settling<'text>
    ) -> Vec<Payload> {
        let sources = [Source::new(PathBuf::from("src/main.rs"), text.to_owned())];
        let resolution = settle(Settling::over("src/main.rs", text)).done();
        let facts = behavior_with(&sources, Some(&resolution));
        let fact = facts
            .iter()
            .find(|fact| fact.function == function)
            .expect("the function is a fact");
        fact.output_writes
            .iter()
            .filter_map(|write| match &write.payload {
                Payload::Template(segments) => Some(segments),
                _ => None
            })
            .flatten()
            .find_map(|segment| match segment {
                Segment::Hole(hole)
                    if matches!(&hole.source,
                        crate::semantic::behavior::HoleSource::Named(name) if name == position) =>
                {
                    Some(hole.alternatives.clone())
                }
                _ => None
            })
            .unwrap_or_default()
    }

    /// The position holding an error is answered by the display
    /// implementation of that error's own type, one entry per write it
    /// records, each proven as far as that write is.
    #[test]
    fn a_printed_error_names_every_text_its_type_can_render() {
        let listed = settled_alternatives(PROGRAM, "main", "error", |settling| {
            settling.typed("error", "Failure")
        });
        assert_eq!(listed.len(), 2);
        assert!(
            matches!(&listed[0], Payload::Template(_)),
            "the first arm writes a shape with a position of its own"
        );
        assert_eq!(listed[1], Payload::Literal("empty".to_owned()));
    }

    /// A position whose type nothing resolved is left as it was: an
    /// empty list means the analysis enumerated nothing, and must never
    /// read as the value rendering nothing.
    #[test]
    fn an_unresolved_position_is_left_unanswered() {
        assert_eq!(
            alternatives(
                "fn main() {\n    let value = compute();\n    println!(\"{value}\");\n}",
                "main",
                "value"
            ),
            [] as [crate::semantic::behavior::output::Payload; 0]
        );
    }

    /// The list is taken from the facts as they stood before any position
    /// was answered, so an entry never carries answers of its own.
    #[test]
    fn the_list_is_one_level_deep() {
        let listed = settled_alternatives(PROGRAM, "main", "error", |settling| {
            settling.typed("error", "Failure")
        });
        for entry in listed {
            let Payload::Template(segments) = entry else {
                continue;
            };
            for segment in segments {
                if let Segment::Hole(hole) = segment {
                    assert!(
                        hole.alternatives.is_empty(),
                        "an entry answered its own position"
                    );
                }
            }
        }
    }
}
