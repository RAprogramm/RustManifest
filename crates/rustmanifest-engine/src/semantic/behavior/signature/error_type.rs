// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The failure a return type declares: the error side of a `Result`, as
//! the compiler rendered the function's own return.
//!
//! Reading it off the written return took whatever the source spelled:
//! an alias hiding both halves answered with the alias, an import
//! renaming the error type answered with the new name, and a return
//! written through a type this workspace does not declare answered with
//! a segment naming nothing. The compiler's rendering carries the type
//! that was reached, and a return it left unsettled declares no failure
//! here rather than a guessed one.

/// The error types a rendered return declares, empty where it declares
/// none.
pub(in crate::semantic::behavior) fn error_types(rendered: Option<&str>) -> Vec<String> {
    let Some(rendered) = rendered else {
        return Vec::new();
    };
    let trimmed = rendered.trim();
    if crate::semantic::behavior::outermost_name(trimmed).as_deref() != Some("Result") {
        return Vec::new();
    }
    let Some(inside) = trimmed.split_once('<').and_then(|(_, rest)| rest.strip_suffix('>')) else {
        return Vec::new();
    };
    arguments(inside)
        .into_iter()
        .skip(1)
        .filter_map(|written| crate::semantic::behavior::outermost_name(&written))
        .collect()
}

/// The top-level arguments of a rendered argument list.
fn arguments(inside: &str) -> Vec<String> {
    let mut written = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (at, letter) in inside.char_indices() {
        match letter {
            '<' | '(' | '[' => depth += 1,
            '>' | ')' | ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                written.push(inside[start..at].trim().to_owned());
                start = at + 1;
            }
            _ => {}
        }
    }
    written.push(inside[start..].trim().to_owned());
    written
}
