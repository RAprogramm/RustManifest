// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a compiler settles about the standard macros a fixture writes.
//!
//! Every reading of a macro answers from the item the invocation
//! reaches, because a program's own macro spelled `println` is not the
//! library's. A fixture of a few lines writes no such namesake, so the
//! written name settles to the library item of that name — and this
//! states it once for the whole file instead of a row per invocation.
//!
//! Stated here rather than in the readers: the readers must keep asking
//! the compiler, and a fixture must keep being able to answer for one.

use rustmanifest_schema::{Location, Range, Resolution, ResolvedPath, ResolvedType};

use crate::source::Source;

/// The library items the standard macros of a fixture reach, by the name
/// each one is written with.
const STANDARD_MACROS: [(&str, &str); 20] = [
    ("assert", "core::macros::builtin::assert"),
    ("assert_eq", "core::macros::assert_eq"),
    ("assert_ne", "core::macros::assert_ne"),
    ("dbg", "std::macros::dbg"),
    ("debug_assert", "core::macros::debug_assert"),
    ("debug_assert_eq", "core::macros::debug_assert_eq"),
    ("debug_assert_ne", "core::macros::debug_assert_ne"),
    ("eprint", "std::macros::eprint"),
    ("eprintln", "std::macros::eprintln"),
    ("format", "alloc::macros::format"),
    ("format_args", "std::macros::format_args"),
    ("panic", "core::macros::panic"),
    ("print", "std::macros::print"),
    ("println", "std::macros::println"),
    ("todo", "core::macros::todo"),
    ("unimplemented", "core::macros::unimplemented"),
    ("unreachable", "core::macros::unreachable"),
    ("vec", "alloc::macros::vec"),
    ("write", "core::macros::write"),
    ("writeln", "core::macros::writeln")
];

/// The macros whose invocation ends the path it stands on, so the
/// compiler types that position as one no value comes back from.
const LEAVING_MACROS: [&str; 4] = [
    "core::macros::panic",
    "core::macros::todo",
    "core::macros::unimplemented",
    "core::macros::unreachable"
];

/// The rendering the compiler gives a position no value comes back from.
const NEVER: &str = "!";

/// Adds to `resolution` what a compiler settles about the standard
/// macros `sources` write.
pub fn settle_standard_macros(resolution: &mut Resolution, sources: &[Source]) {
    for source in sources {
        let text = source.text();
        for (written, names) in STANDARD_MACROS {
            for at in written_at(text, written) {
                resolution.paths.push(ResolvedPath {
                    at:        Location {
                        file:  source.path().to_path_buf(),
                        range: at
                    },
                    names:     names.to_owned(),
                    kind:      "macro".to_owned(),
                    declaring: names.split("::").next().unwrap_or(names).to_owned()
                });
                if LEAVING_MACROS.contains(&names)
                    && let Some(whole) = invocation_at(text, at)
                {
                    resolution.types.push(ResolvedType {
                        at:       Location {
                            file:  source.path().to_path_buf(),
                            range: whole
                        },
                        rendered: NEVER.to_owned(),
                        declared: None
                    });
                }
            }
        }
    }
    resolution.settle();
}

/// What a compiler settles about the standard macros `sources` write,
/// and nothing else.
#[must_use]
pub fn standard_macros_over(sources: &[Source]) -> Resolution {
    let mut resolution = Resolution {
        resolver: "test".to_owned(),
        ..Resolution::default()
    };
    settle_standard_macros(&mut resolution, sources);
    resolution
}

/// Every range the name of a macro invocation stands at: the name
/// itself, followed by the bang that makes it one.
fn written_at(text: &str, written: &str) -> Vec<Range> {
    let mut found = Vec::new();
    let mut from = 0usize;
    while let Some(at) = text.get(from..).and_then(|rest| rest.find(written)) {
        let start = from + at;
        let end = start + written.len();
        from = end;
        if text[end..].starts_with('!')
            && !text[..start]
                .chars()
                .next_back()
                .is_some_and(|letter| letter.is_alphanumeric() || letter == '_')
        {
            found.push(Range {
                start: u32::try_from(start).unwrap_or(u32::MAX),
                end:   u32::try_from(end).unwrap_or(u32::MAX)
            });
        }
    }
    found
}

/// The range the whole invocation written at `name` stands at, its
/// argument list closed.
fn invocation_at(text: &str, name: Range) -> Option<Range> {
    let after = name.end as usize + 1;
    let opening = text.get(after..)?.chars().next()?;
    let closing = match opening {
        '(' => ')',
        '[' => ']',
        '{' => '}',
        _ => return None
    };
    let mut depth = 0usize;
    for (at, letter) in text.get(after..)?.char_indices() {
        if letter == opening {
            depth += 1;
        } else if letter == closing {
            depth -= 1;
            if depth == 0 {
                return Some(Range {
                    start: name.start,
                    end:   u32::try_from(after + at + 1).unwrap_or(u32::MAX)
                });
            }
        }
    }
    None
}
