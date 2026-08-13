// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a function hands back to whoever called it.
//!
//! One record per return position — the body's tail expression, every
//! written `return`, and every `?` — in body order, never deduplicated,
//! each carrying what stands over it. A `?` belongs here because a
//! failure at that position leaves the function as the function's own
//! failure; the completion channel cannot see it, since it recognizes
//! only the four spellings that end the process, and a `?` ends a call.
//!
//! A `return` inside a closure or a nested item is not recorded: it
//! leaves that body, not this one.
//!
//! The value a position hands back is named by origin, so a caller can
//! restate it in terms of what it handed in — the whole of why a summary
//! composes.

use super::{
    output::SiteGuard,
    value::{Condition, Provenance}
};

/// Version of the return positions this channel recognizes. Stamped into
/// claims so a negative result states which recognizer produced it.
pub const HANDBACK_SET: u32 = 1;

/// Which half of a fallible return a position hands back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Outcome {
    /// A value the signature returns outright.
    Plain,
    /// The success half of a fallible return.
    Success,
    /// The failure half of a fallible return.
    Failure,
    /// Which half is handed back is not identified.
    NotIdentified
}

impl Outcome {
    /// The half this position hands back, as a stable key.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::Success => "success",
            Self::Failure => "failure",
            Self::NotIdentified => "not-identified"
        }
    }
}

impl std::fmt::Display for Outcome {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(self.label())
    }
}

/// One position handing a value back to the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handback {
    /// Where the value handed back came from.
    pub value:      Provenance,
    /// Which half of a fallible return it is.
    pub outcome:    Outcome,
    /// Byte range of the construct handing it back.
    pub site:       rustmanifest_schema::Range,
    /// What stands between the position and unconditional execution.
    pub guard:      SiteGuard,
    /// What each construct standing over the position tests, outermost
    /// first, one entry per guard frame.
    pub conditions: Vec<Condition>
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

    use super::Outcome;
    use crate::{
        semantic::behavior::value::{Literal, Provenance},
        source::Source
    };

    fn positions(text: &str, function: &str) -> Vec<(Outcome, Provenance)> {
        let sources = [Source::new(PathBuf::from("src/lib.rs"), text.to_owned())];
        crate::semantic::behavior_with(&sources, Some(&crate::semantic::settling::settled_over(&sources)))
            .iter()
            .find(|fact| fact.function == function)
            .map(|fact| {
                fact.handbacks
                    .iter()
                    .map(|handback| (handback.outcome, handback.value.clone()))
                    .collect()
            })
            .expect("the function is a fact")
    }

    /// A written return and the tail expression are both positions, in
    /// body order, and the value each hands back is named by origin.
    #[test]
    fn every_written_return_position_is_recorded_in_body_order() {
        assert_eq!(
            positions(
                "fn pick(flag: bool) -> u32 { if flag { return 1; } 2 }",
                "pick"
            ),
            vec![
                (
                    Outcome::Plain,
                    Provenance::Written(Literal::Integer("1".to_owned()))
                ),
                (
                    Outcome::Plain,
                    Provenance::Written(Literal::Integer("2".to_owned()))
                )
            ]
        );
    }

    /// A fallible return written as one of its two variants says which
    /// half it hands back.
    #[test]
    fn a_written_variant_names_the_half_it_hands_back() {
        let handed = positions(
            "fn load(flag: bool) -> Result<u32, Fault> { if flag { return Err(Fault); } Ok(1) }",
            "load"
        );
        assert_eq!(
            handed
                .iter()
                .map(|(outcome, _)| *outcome)
                .collect::<Vec<_>>(),
            vec![Outcome::Failure, Outcome::Success]
        );
    }

    /// A `?` hands the failure back to the caller, so it is a return
    /// position of this function even though nothing is written there.
    #[test]
    fn a_question_mark_is_a_return_position() {
        let handed = positions(
            "fn chain() -> Result<u32, Fault> { let value = inner()?; Ok(value) }",
            "chain"
        );
        assert_eq!(handed[0].0, Outcome::Failure);
        assert_eq!(handed.len(), 2);
    }

    /// A body ending in a statement writes no tail position, and none is
    /// invented at the closing brace.
    #[test]
    fn a_body_with_no_tail_expression_records_no_tail_position() {
        assert!(positions("fn shout() { println!(\"hi\"); }", "shout").is_empty());
    }
}
