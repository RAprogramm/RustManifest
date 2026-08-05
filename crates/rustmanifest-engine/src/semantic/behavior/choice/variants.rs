// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Whether a written alternative set names every variant of a standard
//! type.
//!
//! `match value { Ok(v) => .., Err(e) => .. }` runs exactly one alternative
//! and the language guarantees it, but nothing in the fact base enumerates
//! a type's variants, so the tier called such a set unproven — which reads
//! as doubt about the better-written form.
//!
//! The proof here is **syntactic and says so**: it rests on the written
//! constructor names matching a standard type's variant set exactly, not on
//! the scrutinee's type, which this tier does not read. A program that
//! defines its own two-variant type spelled the same way would be credited
//! wrongly, which is why the closure it yields is a distinct answer rather
//! than the one a catch-all earns.

/// Variant sets of the standard types whose alternatives this tier can
/// recognise, each written exactly as the source spells a pattern.
const STANDARD_SETS: [&[&str]; 2] = [&["Err", "Ok"], &["None", "Some"]];

/// Returns whether `patterns` name every variant of one standard type.
///
/// Each pattern is read down to its leading constructor name; anything
/// else — a binding, a wildcard, a tuple, a path of several segments —
/// answers no, because the reading cannot say what it covers.
///
/// A guard is not a pattern and never reaches here: an alternative
/// carrying one is dropped by the caller, because a guarded arm can
/// decline the value its pattern accepts.
pub(in crate::semantic::behavior) fn cover_a_standard_type(patterns: &[&str]) -> bool {
    let mut named: Vec<&str> = patterns.iter().filter_map(|p| constructor(p)).collect();
    if named.len() != patterns.len() {
        return false;
    }
    named.sort_unstable();
    named.dedup();
    STANDARD_SETS.contains(&named.as_slice())
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

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use super::*;

    #[test]
    fn both_halves_of_a_fallible_result_cover_it() {
        assert!(cover_a_standard_type(&["Ok(value)", "Err(_)"]));
        assert!(cover_a_standard_type(&["Err(error)", "Ok(v)"]));
    }

    #[test]
    fn both_halves_of_an_optional_cover_it() {
        assert!(cover_a_standard_type(&["Some(value)", "None"]));
    }

    #[test]
    fn one_half_alone_covers_nothing() {
        assert!(!cover_a_standard_type(&["Ok(value)"]));
        assert!(!cover_a_standard_type(&["Some(value)"]));
    }

    #[test]
    fn halves_of_two_different_types_cover_neither() {
        assert!(!cover_a_standard_type(&["Ok(value)", "None"]));
    }

    #[test]
    fn a_pattern_this_reading_cannot_name_answers_no() {
        assert!(!cover_a_standard_type(&["Ok(value)", "_"]));
        assert!(!cover_a_standard_type(&["Ok(value)", "other"]));
        assert!(!cover_a_standard_type(&[
            "core :: result :: Result :: Ok(v)",
            "Err(_)"
        ]));
    }

    #[test]
    fn a_repeated_half_does_not_stand_in_for_the_other() {
        assert!(!cover_a_standard_type(&["Ok(a)", "Ok(b)"]));
    }
}
