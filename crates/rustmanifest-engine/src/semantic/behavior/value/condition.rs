// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a construct standing over a position tests.
//!
//! The guard stack records the *kind* of an enclosing construct; this
//! records *which* one. Two positions whose condition stacks agree up to
//! one entry, and differ in which side of that entry they stand on, are
//! proven alternatives of each other. The guard stack alone can never
//! prove that: it says "a match arm" of both, and two arms of one match
//! and one arm each of two matches read identically through it.
//!
//! One entry per enclosing guard frame, outermost first, so a record's
//! condition stack is exactly as long as its guard stack. A frame whose
//! test was not read is a stated hole and never a shortened stack, which
//! would make two unrelated positions look like siblings.

use super::provenance::Provenance;

/// What a construct standing over a position tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Condition {
    /// A conditional's test, and whether the position stands on the side
    /// taken when it holds.
    Branch {
        /// Where the tested value came from.
        test:  Provenance,
        /// Whether this side runs when the test holds.
        taken: bool
    },
    /// One alternative of a choosing construct, both numbered in the
    /// order the body writes them.
    Arm {
        /// Which choosing construct of this body.
        choice:      u32,
        /// Which alternative of it.
        alternative: u32
    },
    /// A construct stands here whose test this tier did not read.
    NotDerived
}

impl Condition {
    /// The kind of test, as a stable key.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Branch {
                ..
            } => "branch",
            Self::Arm {
                ..
            } => "arm",
            Self::NotDerived => "not-derived"
        }
    }

    /// Whether two positions standing under these two entries are
    /// alternatives of each other.
    ///
    /// Two alternatives of one and the same choosing construct are, and
    /// the two sides of one test are. Two sides of two tests are only
    /// where the tested value is the same value — which the origin
    /// settles, since a name standing for more than one value refuses
    /// rather than resolving. Everything else is not proven exclusive,
    /// and listing beats joining.
    #[must_use]
    pub fn exclusive_of(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Arm {
                    choice: own,
                    alternative: mine
                },
                Self::Arm {
                    choice: theirs,
                    alternative: other
                }
            ) => own == theirs && mine != other,
            (
                Self::Branch {
                    test: own,
                    taken: mine
                },
                Self::Branch {
                    test: theirs,
                    taken: other
                }
            ) => mine != other && own == theirs && own.derived(),
            _ => false
        }
    }
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

    use super::Condition;
    use crate::source::Source;

    /// The condition stack of every return position of one function,
    /// paired with the length of the guard stack beside it.
    fn stacks(text: &str, function: &str) -> Vec<(Vec<Condition>, usize)> {
        let sources = [Source::new(PathBuf::from("src/lib.rs"), text.to_owned())];
        crate::semantic::behavior_with(
            &sources,
            Some(&crate::semantic::settling::settled_over(&sources))
        )
        .iter()
        .find(|fact| fact.function == function)
        .map(|fact| {
            fact.handbacks
                .iter()
                .map(|position| (position.conditions.clone(), position.guard.guards.len()))
                .collect()
        })
        .expect("the function is a fact")
    }

    /// The two stacks are the same length, entry for entry: a frame whose
    /// test was not read leaves a stated hole, never a shorter stack that
    /// would make two unrelated positions read as siblings.
    #[test]
    fn the_condition_stack_is_as_long_as_the_guard_stack() {
        let recorded = stacks(
            "fn pick(flag: bool, other: bool) -> u32 {\n    if flag { for _ in 0..2 { if other { \
             return 1; } } }\n    2\n}",
            "pick"
        );
        assert_ne!(
            recorded,
            [] as [(
                std::vec::Vec<crate::semantic::behavior::value::condition::Condition>,
                usize
            ); 0]
        );
        for (conditions, guards) in recorded {
            assert_eq!(conditions.len(), guards, "{conditions:?}");
        }
    }

    /// Two positions under two alternatives of one choosing construct are
    /// alternatives of each other; the guard stack alone could never say
    /// so, since it names the kind of the construct and not which one.
    #[test]
    fn two_alternatives_of_one_choice_are_alternatives_of_each_other() {
        let recorded = stacks(
            "fn pick(value: Choice) -> u32 {\n    match value {\n        Choice::One => return 1,\n \
             Choice::Two => return 2,\n        _ => 3\n    }\n}",
            "pick"
        );
        let [(first, _), (second, _), ..] = recorded.as_slice() else {
            panic!("two written returns stand under two arms: {recorded:?}");
        };
        assert!(first[0].exclusive_of(&second[0]), "{first:?} {second:?}");
    }

    /// One alternative each of two choosing constructs proves nothing:
    /// both can run in one execution, so they are listed and not joined.
    #[test]
    fn one_alternative_each_of_two_choices_is_not_exclusive() {
        let recorded = stacks(
            "fn pick(one: Choice, other: Choice) -> u32 {\n    match one { Choice::One => return \
             1, _ => () }\n    match other { Choice::One => return 2, _ => () }\n    3\n}",
            "pick"
        );
        let [(first, _), (second, _), ..] = recorded.as_slice() else {
            panic!("two written returns stand under two matches: {recorded:?}");
        };
        assert!(!first[0].exclusive_of(&second[0]), "{first:?} {second:?}");
    }

    /// The two sides of one test are alternatives; the tested value is
    /// named by origin, so two tests of two different values are not.
    #[test]
    fn the_two_sides_of_one_test_are_alternatives() {
        let recorded = stacks(
            "fn pick(flag: bool) -> u32 { if flag { return 1; } else { return 2; } }",
            "pick"
        );
        let [(first, _), (second, _), ..] = recorded.as_slice() else {
            panic!("both sides are written: {recorded:?}");
        };
        assert!(first[0].exclusive_of(&second[0]), "{first:?} {second:?}");
    }
}
