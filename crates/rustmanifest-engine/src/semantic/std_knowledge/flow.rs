// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Where the value a library call hands back comes from, stated as rows.
//!
//! This is the shape the industry settled on for describing code the
//! analyzer does not have: a table saying, per method, which of its inputs
//! the returned value carries. A reading that walks bodies cannot walk a
//! body it does not have, and guessing is the alternative nobody should
//! take — so the guarantee is written down instead, one row at a time.
//!
//! **Only value-preserving rows are admitted.** The same tables elsewhere
//! also carry "taint" rows, meaning a value derived somehow from an input;
//! that is an over-approximation, and an over-approximation in a chain the
//! engine states as fact would make the sentence claim more than it knows.
//! A method whose answer is derived rather than carried is simply absent
//! here, and the reading refuses at it by name.
//!
//! What a row cannot say, it does not say. A method whose answer depends
//! on something written nowhere — an element chosen at run time — has no
//! row, and the reading refuses at it by name. A fallback to the type's
//! own default is the one admitted exception: the default's value is
//! written elsewhere, but that the position falls back to that
//! declaration is the library's guarantee, and the row names the
//! declaration rather than claiming the value.

/// Which input a library call hands back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::semantic) enum Carried {
    /// The receiver itself: the call answers with what it was given.
    Receiver,
    /// What the receiver holds inside it.
    Held,
    /// Whatever the receiver holds, or the argument at this position
    /// where it holds nothing.
    HeldOrArgument(u8),
    /// Whatever the receiver holds, or the default its type declares
    /// where it holds nothing.
    HeldOrDefault,
    /// Whatever the closure written at this position hands back, applied
    /// to what the receiver holds.
    ///
    /// Two steps of one row: what the receiver holds reaches the closure's
    /// input, and what the closure hands back reaches the call's answer.
    /// Both are guaranteed by the library, and neither is an
    /// approximation.
    ClosureOver(u8)
}

/// The bodies whose answer carries one of their inputs, each named by
/// the item the call reaches and paired with which input it carries.
///
/// Written method names keyed this before, and keyed it for a workspace
/// method spelled the same way just as readily: the compiler settles
/// which body a call reaches, and only that body carries the library's
/// guarantee.
const FLOW_METHODS: [(&str, Carried); 28] = [
    ("core::result::Result::unwrap_or_default", Carried::HeldOrDefault),
    ("core::option::Option::unwrap_or_default", Carried::HeldOrDefault),
    ("core::result::Result::map_err", Carried::Receiver),
    ("core::option::Option::ok_or", Carried::Receiver),
    ("core::option::Option::ok_or_else", Carried::Receiver),
    ("core::result::Result::ok", Carried::Receiver),
    ("core::option::Option::as_ref", Carried::Receiver),
    ("core::result::Result::as_ref", Carried::Receiver),
    ("core::option::Option::as_mut", Carried::Receiver),
    ("core::result::Result::as_mut", Carried::Receiver),
    ("core::option::Option::as_deref", Carried::Receiver),
    ("core::option::Option::cloned", Carried::Receiver),
    ("core::option::Option::copied", Carried::Receiver),
    ("core::clone::Clone::clone", Carried::Receiver),
    ("core::option::Option::clone", Carried::Receiver),
    ("core::result::Result::clone", Carried::Receiver),
    ("alloc::str::to_owned", Carried::Receiver),
    ("alloc::borrow::ToOwned::to_owned", Carried::Receiver),
    ("core::option::Option::unwrap", Carried::Held),
    ("core::result::Result::unwrap", Carried::Held),
    ("core::option::Option::expect", Carried::Held),
    ("core::result::Result::expect", Carried::Held),
    ("core::option::Option::unwrap_or", Carried::HeldOrArgument(0)),
    ("core::result::Result::unwrap_or", Carried::HeldOrArgument(0)),
    ("core::option::Option::and_then", Carried::ClosureOver(0)),
    ("core::result::Result::and_then", Carried::ClosureOver(0)),
    ("core::option::Option::map", Carried::ClosureOver(0)),
    ("core::result::Result::map", Carried::ClosureOver(0))
];

/// What the body a call reaches hands back, where a row names it.
pub(in crate::semantic) fn carried_by(reached: &str) -> Option<Carried> {
    FLOW_METHODS
        .iter()
        .find(|(named, _)| *named == reached)
        .map(|(_, carried)| *carried)
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
    fn an_adapter_answers_with_what_it_was_given() {
        assert_eq!(carried_by("core::result::Result::map_err"), Some(Carried::Receiver));
        assert_eq!(carried_by("core::option::Option::cloned"), Some(Carried::Receiver));
    }

    #[test]
    fn taking_a_value_out_answers_with_what_was_held() {
        assert_eq!(carried_by("core::option::Option::unwrap"), Some(Carried::Held));
        assert_eq!(carried_by("core::option::Option::expect"), Some(Carried::Held));
    }

    #[test]
    fn a_fallback_answers_with_one_or_the_other() {
        assert_eq!(carried_by("core::option::Option::unwrap_or"), Some(Carried::HeldOrArgument(0)));
    }

    /// A method whose answer is computed from an input rather than carried
    /// has no row: the sentence must refuse at it rather than credit it
    /// with a value it never handed on.
    #[test]
    fn a_combinator_answers_with_what_its_closure_hands_back() {
        assert_eq!(carried_by("core::option::Option::and_then"), Some(Carried::ClosureOver(0)));
        assert_eq!(carried_by("core::option::Option::map"), Some(Carried::ClosureOver(0)));
    }

    #[test]
    fn a_method_whose_answer_is_written_nowhere_has_no_row() {
        assert_eq!(carried_by("filter"), None);
        assert_eq!(carried_by("len"), None);
    }

    /// The fallback names the declaration, never the value: the row says
    /// the answer is the held value or what `Default` declares, and which
    /// bytes that declaration amounts to stays unclaimed.
    #[test]
    fn a_fallback_to_the_declared_default_names_the_declaration() {
        assert_eq!(
            carried_by("core::result::Result::unwrap_or_default"),
            Some(Carried::HeldOrDefault)
        );
    }
}
