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

use rustmanifest_schema::{Location, Range, Resolution, ResolvedCall, ResolvedPath, ResolvedType};

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

/// The library containers a fixture allocates, with the crate that
/// declares each — the crate is what makes a call into it an allocation.
const ALLOCATED: [(&str, &str); 5] = [
    ("String", "alloc::string::String"),
    ("Vec", "alloc::vec::Vec"),
    ("VecDeque", "alloc::collections::VecDeque"),
    ("Box", "alloc::boxed::Box"),
    ("Rc", "alloc::rc::Rc")
];

/// The constructors a fixture writes such a container with.
const CONSTRUCTORS: [&str; 3] = ["new", "with_capacity", "from"];

/// The outcome variants a fixture writes, each with the item it
/// reaches: which one a written name means is the compiler's answer,
/// and a fixture writes no namesake of its own.
const OUTCOME_VARIANTS: [(&str, &str); 4] = [
    ("Ok", "core::result::Ok"),
    ("Err", "core::result::Err"),
    ("Some", "core::option::Some"),
    ("None", "core::option::None")
];

/// The bodies that end the process, by the path a fixture writes them
/// with.
const ENDING: [(&str, &str); 4] = [
    ("std::process::exit", "std::process::exit"),
    ("process::exit", "std::process::exit"),
    ("std::process::abort", "std::process::abort"),
    ("process::abort", "std::process::abort")
];

/// The process's own handle constructors a fixture writes, with the
/// body each reaches and the type it hands back.
const HANDLES: [(&str, &str, &str); 3] = [
    ("std::io::stdout", "std::io::stdio::stdout", "std::io::stdio::Stdout"),
    ("std::io::stderr", "std::io::stdio::stderr", "std::io::stdio::Stderr"),
    ("std::io::stdin", "std::io::stdio::stdin", "std::io::stdio::Stdin")
];

/// The numeric methods a fixture writes, each reaching the numeric
/// module of the language's own library.
const NUMERIC: [&str; 20] = [
    "checked_add",
    "checked_div",
    "checked_mul",
    "checked_neg",
    "checked_rem",
    "checked_sub",
    "overflowing_add",
    "overflowing_mul",
    "overflowing_sub",
    "saturating_add",
    "saturating_mul",
    "saturating_sub",
    "wrapping_add",
    "wrapping_div",
    "wrapping_mul",
    "wrapping_neg",
    "wrapping_rem",
    "wrapping_sub",
    "abs_diff",
    "pow"
];

/// The methods that hand back a copy of what they are written on, which
/// the compiler states as a shared borrow answering with the receiver's
/// own type.
const COPYING: [&str; 3] = ["clone", "to_owned", "to_vec"];

/// The rendering a fixture states for a copy's receiver and answer
/// alike: the two are one type, which is the whole of what the reading
/// asks, and naming it would state more than the fixture knows.
const SAME_TYPE: &str = "Self";

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
    settle_allocations(resolution, sources);
    settle_copies(resolution, sources);
    settle_variants(resolution, sources);
    settle_endings(resolution, sources);
    settle_handles(resolution, sources);
    settle_numeric(resolution, sources);
    resolution.settle();
}

/// Records the constructor calls a fixture writes into the allocating
/// crate, so a reading asking which crate a call reaches is answered.
fn settle_allocations(resolution: &mut Resolution, sources: &[Source]) {
    for source in sources {
        let text = source.text();
        for (written, declared) in ALLOCATED {
            for constructor in CONSTRUCTORS {
                let spelled = format!("{written}::{constructor}");
                for at in call_at(text, &spelled) {
                    resolution.calls.push(ResolvedCall {
                        at:             Location {
                            file:  source.path().to_path_buf(),
                            range: at
                        },
                        callee:         constructor.to_owned(),
                        returns:        written.to_owned(),
                        receiver:       None,
                        receiver_path:  None,
                        caller_decides: false,
                        reaches:        Some(format!("{declared}::{constructor}")),
                        takes:          None
                    });
                }
            }
        }
    }
}

/// Records the outcome variants a fixture writes, as the items they
/// reach.
fn settle_variants(resolution: &mut Resolution, sources: &[Source]) {
    for source in sources {
        let text = source.text();
        for (written, names) in OUTCOME_VARIANTS {
            for at in name_at(text, written) {
                resolution.paths.push(ResolvedPath {
                    at:        Location {
                        file:  source.path().to_path_buf(),
                        range: at
                    },
                    names:     names.to_owned(),
                    kind:      "variant".to_owned(),
                    declaring: "core".to_owned()
                });
            }
        }
    }
}

/// Records the process-ending calls a fixture writes, as the bodies
/// they reach.
fn settle_endings(resolution: &mut Resolution, sources: &[Source]) {
    for source in sources {
        let text = source.text();
        for (written, names) in ENDING {
            for at in call_at(text, written) {
                resolution.calls.push(ResolvedCall {
                    at:             Location {
                        file:  source.path().to_path_buf(),
                        range: at
                    },
                    callee:         names.rsplit("::").next().unwrap_or(names).to_owned(),
                    returns:        NEVER.to_owned(),
                    receiver:       None,
                    receiver_path:  None,
                    caller_decides: false,
                    reaches:        Some(names.to_owned()),
                    takes:          None
                });
            }
        }
    }
}

/// Records the handle constructors a fixture writes, as the bodies they
/// reach and the types they hand back.
fn settle_handles(resolution: &mut Resolution, sources: &[Source]) {
    for source in sources {
        let text = source.text();
        for (written, reaches, declared) in HANDLES {
            for at in call_at(text, written) {
                resolution.calls.push(ResolvedCall {
                    at:             Location {
                        file:  source.path().to_path_buf(),
                        range: at
                    },
                    callee:         reaches.rsplit("::").next().unwrap_or(reaches).to_owned(),
                    returns:        declared.rsplit("::").next().unwrap_or(declared).to_owned(),
                    receiver:       None,
                    receiver_path:  None,
                    caller_decides: false,
                    reaches:        Some(reaches.to_owned()),
                    takes:          None
                });
                resolution.types.push(ResolvedType {
                    at:       Location {
                        file:  source.path().to_path_buf(),
                        range: at
                    },
                    rendered: declared.rsplit("::").next().unwrap_or(declared).to_owned(),
                    declared: Some(declared.to_owned())
                });
            }
        }
    }
}

/// Records the numeric calls a fixture writes, as the bodies of the
/// numeric module they reach.
fn settle_numeric(resolution: &mut Resolution, sources: &[Source]) {
    for source in sources {
        let text = source.text();
        for method in NUMERIC {
            let spelled = format!(".{method}");
            let mut from = 0usize;
            while let Some(at) = text.get(from..).and_then(|rest| rest.find(&spelled)) {
                let start = from + at;
                let opens = start + spelled.len();
                from = opens;
                let Some(whole) = written_call(text, opens) else {
                    continue;
                };
                resolution.calls.push(ResolvedCall {
                    at:             Location {
                        file:  source.path().to_path_buf(),
                        range: whole
                    },
                    callee:         method.to_owned(),
                    returns:        String::new(),
                    receiver:       None,
                    receiver_path:  None,
                    caller_decides: false,
                    reaches:        Some(format!("core::num::{method}")),
                    takes:          None
                });
            }
        }
    }
}

/// Every range a bare written name stands at, where nothing joins it to
/// a longer one.
fn name_at(text: &str, written: &str) -> Vec<Range> {
    let mut found = Vec::new();
    let mut from = 0usize;
    while let Some(at) = text.get(from..).and_then(|rest| rest.find(written)) {
        let start = from + at;
        let end = start + written.len();
        from = end;
        let joined_before = text[..start]
            .chars()
            .next_back()
            .is_some_and(|letter| letter.is_alphanumeric() || letter == '_');
        let joined_after = text[end..]
            .chars()
            .next()
            .is_some_and(|letter| letter.is_alphanumeric() || letter == '_');
        if !joined_before && !joined_after {
            found.push(Range {
                start: u32::try_from(start).unwrap_or(u32::MAX),
                end:   u32::try_from(end).unwrap_or(u32::MAX)
            });
        }
    }
    found
}

/// Records the copying calls a fixture writes, as the compiler states
/// them: taken by shared borrow, answering with the receiver's own type.
fn settle_copies(resolution: &mut Resolution, sources: &[Source]) {
    for source in sources {
        let text = source.text();
        for method in COPYING {
            let spelled = format!(".{method}");
            let mut from = 0usize;
            while let Some(at) = text.get(from..).and_then(|rest| rest.find(&spelled)) {
                let start = from + at;
                let opens = start + spelled.len();
                from = opens;
                let Some(whole) = written_call(text, opens) else {
                    continue;
                };
                resolution.calls.push(ResolvedCall {
                    at:             Location {
                        file:  source.path().to_path_buf(),
                        range: whole
                    },
                    callee:         method.to_owned(),
                    returns:        SAME_TYPE.to_owned(),
                    receiver:       Some(SAME_TYPE.to_owned()),
                    receiver_path:  None,
                    caller_decides: false,
                    reaches:        Some(format!("core::clone::Clone::{method}")),
                    takes:          Some(rustmanifest_schema::ResolvedMode::Borrowed)
                });
            }
        }
    }
}

/// The range the whole call written on a receiver stands at: from the
/// start of the receiver's own expression to the closing bracket.
fn written_call(text: &str, opens: usize) -> Option<Range> {
    let closes = closed_at(text, 0, opens)?.end as usize;
    let head = text.get(..opens)?;
    let start = head
        .char_indices()
        .rev()
        .take_while(|(_, letter)| {
            letter.is_alphanumeric() || *letter == '_' || *letter == '.' || *letter == ')'
        })
        .last()
        .map_or(opens, |(at, _)| at);
    Some(Range {
        start: u32::try_from(start).unwrap_or(u32::MAX),
        end:   u32::try_from(closes).unwrap_or(u32::MAX)
    })
}

/// Every range a written call stands at, its argument list closed.
fn call_at(text: &str, spelled: &str) -> Vec<Range> {
    let mut found = Vec::new();
    let mut from = 0usize;
    while let Some(at) = text.get(from..).and_then(|rest| rest.find(spelled)) {
        let start = from + at;
        let opens = start + spelled.len();
        from = opens;
        let Some(range) = closed_at(text, start, opens) else {
            continue;
        };
        found.push(range);
    }
    found
}

/// The range a call starting at `start` stands at, where its argument
/// list opens at `opens` and closes further on.
fn closed_at(text: &str, start: usize, opens: usize) -> Option<Range> {
    if !text.get(opens..)?.starts_with('(') {
        return None;
    }
    let mut depth = 0usize;
    for (at, letter) in text.get(opens..)?.char_indices() {
        if letter == '(' {
            depth += 1;
        } else if letter == ')' {
            depth -= 1;
            if depth == 0 {
                return Some(Range {
                    start: u32::try_from(start).unwrap_or(u32::MAX),
                    end:   u32::try_from(opens + at + 1).unwrap_or(u32::MAX)
                });
            }
        }
    }
    None
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
