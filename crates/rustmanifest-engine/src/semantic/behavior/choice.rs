// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a program chooses between: one record per choosing construct.
//!
//! A choice is where a program's behaviour forks on a value, and the
//! alternatives are what it can do instead of one another. Recording them
//! is what lets two positions be called alternatives of each other rather
//! than merely both possible — the difference between a statement that
//! joins two outcomes and one that lists them.
//!
//! Whether the alternatives are the whole of what can happen is a
//! separate question from what they are, and it is answered separately: a
//! set closed by an alternative accepting every remaining value is
//! closed, and one closed by naming every variant of a type is recorded
//! as unproven, because nothing here enumerates a type's variants.

mod alternative;
mod site;
mod variants;

pub(in crate::semantic::behavior) use self::variants::cover_a_standard_type;
pub use self::{
    alternative::{Alternative, Closure},
    site::Choice
};

/// Version of the choosing constructs this channel recognizes. Stamped
/// into claims so a negative result states which recognizer produced it.
pub const CHOICE_SET: u32 = 1;

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use std::path::PathBuf;

    use super::{Choice, Closure};
    use crate::source::Source;

    fn chosen(body: &str) -> Vec<Choice> {
        let text = format!("fn decide(value: Kind) -> u32 {{\n{body}\n}}");
        let sources = [Source::new(PathBuf::from("src/lib.rs"), text)];
        crate::semantic::behavior_with(
            &sources,
            Some(&crate::semantic::settling::settled_over(&sources))
        )
        .iter()
        .find(|fact| fact.function == "decide")
        .map(|fact| fact.choices.clone())
        .expect("the function is a fact")
    }

    /// One record per choosing construct, with one alternative per arm
    /// the source writes, in written order.
    #[test]
    fn a_choice_records_one_alternative_per_written_arm() {
        let recorded = chosen(
            "    match value {\n        Kind::One => 1,\n        Kind::Two => \
                               2,\n        _ => 0\n    }"
        );
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].alternatives.len(), 3);
        assert!(recorded[0].alternatives[0].pattern.is_some());
    }

    /// An alternative accepting every remaining value closes the set;
    /// naming every variant of a type does not, because nothing here
    /// enumerates a type's variants and this tier will not pretend to.
    #[test]
    fn the_set_is_closed_only_by_an_alternative_accepting_everything() {
        let with_catch_all =
            chosen("    match value {\n        Kind::One => 1,\n        _ => 0\n    }");
        assert_eq!(with_catch_all[0].closure, Closure::CatchAll);

        let by_variants =
            chosen("    match value {\n        Kind::One => 1,\n        Kind::Two => 2\n    }");
        assert_eq!(
            by_variants[0].closure,
            Closure::Unproven,
            "the better-written form is the one this tier is less certain about, and it says so"
        );
    }

    /// The value being chosen on is named by where it came from, so a
    /// choice on something read from outside the process says so.
    #[test]
    fn the_value_chosen_on_is_named_by_its_origin() {
        let recorded = chosen("    match value {\n        Kind::One => 1,\n        _ => 0\n    }");
        assert!(
            matches!(
                &recorded[0].scrutinee,
                crate::semantic::behavior::Provenance::Parameter(slot) if slot.position == 0
            ),
            "{:?}",
            recorded[0].scrutinee
        );
    }

    /// Each alternative names where the value it produces came from.
    #[test]
    fn each_alternative_names_the_value_it_produces() {
        let recorded = chosen("    match value {\n        Kind::One => 7,\n        _ => 0\n    }");
        assert_eq!(
            recorded[0].alternatives[0].value,
            crate::semantic::behavior::Provenance::Written(
                crate::semantic::behavior::Literal::Integer("7".to_owned())
            )
        );
    }
}
