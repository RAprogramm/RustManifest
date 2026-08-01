// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! `deceptive-doc` check: documentation claims contradicted by the body.
//!
//! Joins doc facts with behavioral facts by qualified name and derives
//! contradictions purely logically. First covered lie: the summary line
//! opens with a read-promise verb (`returns`/`gets`) while the body
//! provably writes state, prints, or spawns processes. The code is the
//! source of truth; the doc is what gets corrected.

use rustmanifest_schema::TraceStep;

use super::{
    behavior::{Effect, behavior},
    docs::doc_facts,
    insight::Insight
};
use crate::source::Source;

/// Doc summary verbs that promise pure observation.
const READ_CLAIM_VERBS: [&str; 2] = ["gets", "returns"];

/// Effects that contradict a read claim, mirroring `deceptive-name`.
const LIE_EFFECTS: [Effect; 2] = [Effect::Process, Effect::ConsoleIo];

/// Reports documented functions whose summary claims a read while the
/// body provably mutates or emits.
pub(super) fn insights(sources: &[Source]) -> Vec<Insight> {
    let behaviors = behavior(sources);
    let mut found = Vec::new();
    for doc in doc_facts(sources) {
        let Some(verb) = doc.summary_verb.as_deref() else {
            continue;
        };
        if !READ_CLAIM_VERBS.contains(&verb) {
            continue;
        }
        let Some(fact) = behaviors
            .iter()
            .find(|fact| fact.function == doc.function)
        else {
            continue;
        };
        if fact.trait_impl || fact.test {
            continue;
        }
        let effects: Vec<String> = fact
            .effects
            .iter()
            .filter(|effect| LIE_EFFECTS.contains(effect))
            .map(ToString::to_string)
            .collect();
        if fact.writes.is_empty() && effects.is_empty() {
            continue;
        }
        let observed = if fact.writes.is_empty() {
            format!("performs effects [{}]", effects.join(", "))
        } else {
            format!("writes [{}]", fact.writes.join(", "))
        };
        let detail = format!(
            "doc of `{}` claims a read (summary starts with `{verb}`) but the body {observed}; \
             the code is the source of truth — fix the documentation",
            doc.function
        );
        let trace = vec![
            TraceStep {
                fact:   format!("doc summary of `{}` opens with `{verb}`", doc.function),
                reason: "the verb claims pure observation".to_owned()
            },
            TraceStep {
                fact:   format!("body of `{}` {observed}", doc.function),
                reason: "observed in the behavioral facts of the body".to_owned()
            },
            TraceStep {
                fact:   "the claim and the observation contradict".to_owned(),
                reason: "a documented read must not write state, print, or spawn processes"
                    .to_owned()
            },
        ];
        found.push(Insight::new(doc.source, doc.range, detail, trace));
    }
    found
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

    use super::insights;
    use crate::source::Source;

    fn found(text: &str) -> usize {
        insights(&[Source::new(PathBuf::from("src/lib.rs"), text.to_owned())]).len()
    }

    #[test]
    fn doc_claiming_a_read_over_a_writing_body_is_a_lie() {
        assert_eq!(
            found(
                "struct Counter { hits: u64 }\nimpl Counter {\n    /// Returns the hit \
                 count.\n    fn hits(&mut self) -> u64 { self.hits += 1; self.hits }\n}"
            ),
            1
        );
    }

    #[test]
    fn honest_read_doc_passes() {
        assert_eq!(
            found(
                "struct Counter { hits: u64 }\nimpl Counter {\n    /// Returns the hit \
                 count.\n    fn hits(&self) -> u64 { self.hits }\n}"
            ),
            0
        );
    }

    #[test]
    fn non_read_summaries_are_not_judged() {
        assert_eq!(
            found(
                "struct Counter { hits: u64 }\nimpl Counter {\n    /// Increments the \
                 counter.\n    fn bump(&mut self) { self.hits += 1; }\n}"
            ),
            0
        );
    }

    #[test]
    fn undocumented_functions_are_not_judged() {
        assert_eq!(
            found(
                "struct Counter { hits: u64 }\nimpl Counter { fn bump(&mut self) { self.hits += \
                 1; } }"
            ),
            0
        );
    }
}
