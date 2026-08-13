// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The question a library call asks about a value that may or may not be
//! there, and the spellings that ask it.
//!
//! `option.is_some()` is not an opaque call: the library guarantees it
//! answers whether the receiver holds a value, and that guarantee is a fact
//! about the program exactly as an addition is. Recording it as an
//! operation over the receiver is what lets the origin of the answer reach
//! back to whatever produced the receiver, instead of stopping at a call
//! the workspace does not define.
//!
//! Four questions, not one: whether a value is held is a different
//! statement from whether it is absent, and collapsing them would put a
//! claim in the sentence that the source did not write.
//!
//! The question lives here, with the knowledge of the library that answers
//! it, and not with the tier that consumes it — a recognizer reading the
//! library must not have to reach back into what reads the recognizer.
//!
//! Rows, not branches, and each row names the item the call reaches
//! rather than the letters somebody typed. A call the compiler settles
//! elsewhere is not read as a presence check, so a program's own
//! `is_some` on its own type is not credited with the library's answer.

/// What a presence check asks of its receiver.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Presence {
    /// Whether a value is held.
    Held,
    /// Whether no value is held.
    Absent,
    /// Whether an attempt succeeded.
    Succeeded,
    /// Whether an attempt failed.
    Failed
}

impl std::fmt::Display for Presence {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(match self {
            Self::Held => "a check that a value is held",
            Self::Absent => "a check that no value is held",
            Self::Succeeded => "a check that an attempt succeeded",
            Self::Failed => "a check that an attempt failed"
        })
    }
}

/// The bodies asking one question about their receiver, each named by
/// the item the call reaches.
const PRESENCE_METHODS: [(&str, Presence); 4] = [
    ("core::option::Option::is_some", Presence::Held),
    ("core::option::Option::is_none", Presence::Absent),
    ("core::result::Result::is_ok", Presence::Succeeded),
    ("core::result::Result::is_err", Presence::Failed)
];

/// The question the body a call reaches asks of its receiver, where it
/// is one of the four.
pub(in crate::semantic) fn presence_method(reached: &str) -> Option<Presence> {
    PRESENCE_METHODS
        .iter()
        .find(|(named, _)| *named == reached)
        .map(|(_, question)| *question)
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
    fn each_question_reads_as_the_one_it_asks() {
        assert_eq!(Presence::Held.to_string(), "a check that a value is held");
        assert_eq!(
            Presence::Absent.to_string(),
            "a check that no value is held"
        );
        assert_eq!(
            Presence::Succeeded.to_string(),
            "a check that an attempt succeeded"
        );
        assert_eq!(
            Presence::Failed.to_string(),
            "a check that an attempt failed"
        );
    }

    #[test]
    fn holding_and_being_absent_are_not_the_same_answer() {
        assert_ne!(Presence::Held, Presence::Absent);
        assert_ne!(Presence::Succeeded, Presence::Failed);
    }

    #[test]
    fn each_written_spelling_names_its_own_question() {
        assert_eq!(presence_method("is_some"), Some(Presence::Held));
        assert_eq!(presence_method("is_none"), Some(Presence::Absent));
        assert_eq!(presence_method("is_ok"), Some(Presence::Succeeded));
        assert_eq!(presence_method("is_err"), Some(Presence::Failed));
    }

    #[test]
    fn a_method_no_row_names_is_no_presence_check() {
        assert_eq!(presence_method("is_empty"), None);
        assert_eq!(presence_method("len"), None);
        assert_eq!(presence_method("is_someone"), None);
    }
}
