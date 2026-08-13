// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Whether a written alternative set names every variant of the type
//! being decided over.
//!
//! `match value { Ok(v) => .., Err(e) => .. }` runs exactly one
//! alternative and the language guarantees it, but nothing in the fact
//! base enumerates a type's variants, so the tier called such a set
//! unproven — which reads as doubt about the better-written form.
//!
//! The proof rests on the type the compiler settled for the scrutinee:
//! the outcome types of the language carry two variants and nothing
//! else, so two distinct unguarded constructors name both of them. A
//! program defining its own two-variant type spelled the same way was
//! credited wrongly while the written names alone answered; its type is
//! not one of these, and it no longer is.

/// Outcome types of the language, each carrying exactly two variants.
const TWO_VARIANT_OUTCOMES: [&str; 2] = ["Option", "Result"];

/// Returns whether `patterns` name every variant of the type the
/// compiler settled for the value being decided over.
///
/// Each pattern is read down to its leading constructor name; anything
/// else — a binding, a wildcard, a tuple, a path of several segments —
/// answers no, because the reading cannot say what it covers.
///
/// A guard is not a pattern and never reaches here: an alternative
/// carrying one is dropped by the caller, because a guarded arm can
/// decline the value its pattern accepts.
pub(in crate::semantic::behavior) fn cover_a_standard_type(
    over: Option<&str>,
    patterns: &[&str]
) -> bool {
    let Some(named) = over.and_then(crate::semantic::behavior::outermost_name) else {
        return false;
    };
    if !TWO_VARIANT_OUTCOMES.contains(&named.as_str()) {
        return false;
    }
    let mut written: Vec<&str> = patterns.iter().filter_map(|p| constructor(p)).collect();
    if written.len() != patterns.len() {
        return false;
    }
    written.sort_unstable();
    written.dedup();
    written.len() == 2
}

/// The leading constructor name a pattern is written with, where it is
/// written as one bare name possibly applied to something.
fn constructor(pattern: &str) -> Option<&str> {
    let head = pattern.split(['(', '{', ' ']).next()?.trim();
    let named = head
        .chars()
        .next()
        .is_some_and(|letter| letter.is_ascii_uppercase())
        && head.chars().all(|letter| letter.is_ascii_alphanumeric());
    named.then_some(head)
}
