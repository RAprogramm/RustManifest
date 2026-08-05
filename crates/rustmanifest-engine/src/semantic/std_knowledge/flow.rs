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
//! on something written nowhere — a type's own default, an element chosen
//! at run time — has no row, and the reading refuses at it by name.

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
    /// Whatever the closure written at this position hands back, applied
    /// to what the receiver holds.
    ///
    /// Two steps of one row: what the receiver holds reaches the closure's
    /// input, and what the closure hands back reaches the call's answer.
    /// Both are guaranteed by the library, and neither is an
    /// approximation.
    ClosureOver(u8)
}

/// Library methods whose answer carries one of their inputs, with which.
///
/// Read as one row of a models-as-data table: the input named here, the
/// return value as the output, and value preservation as the kind.
const FLOW_METHODS: [(&str, Carried); 16] = [
    ("map_err", Carried::Receiver),
    ("ok_or", Carried::Receiver),
    ("ok_or_else", Carried::Receiver),
    ("ok", Carried::Receiver),
    ("as_ref", Carried::Receiver),
    ("as_mut", Carried::Receiver),
    ("as_deref", Carried::Receiver),
    ("cloned", Carried::Receiver),
    ("copied", Carried::Receiver),
    ("clone", Carried::Receiver),
    ("to_owned", Carried::Receiver),
    ("unwrap", Carried::Held),
    ("expect", Carried::Held),
    ("unwrap_or", Carried::HeldOrArgument(0)),
    ("and_then", Carried::ClosureOver(0)),
    ("map", Carried::ClosureOver(0))
];

/// What a library call hands back, where a row names it.
pub(in crate::semantic) fn carried_by(method: &str) -> Option<Carried> {
    FLOW_METHODS
        .iter()
        .find(|(named, _)| *named == method)
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
        assert_eq!(carried_by("map_err"), Some(Carried::Receiver));
        assert_eq!(carried_by("cloned"), Some(Carried::Receiver));
    }

    #[test]
    fn taking_a_value_out_answers_with_what_was_held() {
        assert_eq!(carried_by("unwrap"), Some(Carried::Held));
        assert_eq!(carried_by("expect"), Some(Carried::Held));
    }

    #[test]
    fn a_fallback_answers_with_one_or_the_other() {
        assert_eq!(carried_by("unwrap_or"), Some(Carried::HeldOrArgument(0)));
    }

    /// A method whose answer is computed from an input rather than carried
    /// has no row: the sentence must refuse at it rather than credit it
    /// with a value it never handed on.
    #[test]
    fn a_combinator_answers_with_what_its_closure_hands_back() {
        assert_eq!(carried_by("and_then"), Some(Carried::ClosureOver(0)));
        assert_eq!(carried_by("map"), Some(Carried::ClosureOver(0)));
    }

    #[test]
    fn a_method_whose_answer_is_written_nowhere_has_no_row() {
        assert_eq!(carried_by("filter"), None);
        assert_eq!(carried_by("unwrap_or_default"), None);
        assert_eq!(carried_by("len"), None);
    }
}
