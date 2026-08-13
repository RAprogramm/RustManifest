// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Name semantics as logical facts: what a function's name promises,
//! derived from its leading verb, checked against what its body provably
//! does.
//!
//! The verb taxonomy is data, not opinion: a name starting with a
//! read-verb (`get_`, `is_`, `has_`, `peek_`, `len`) promises observation
//! — no field writes, no outside-world effects. The `deceptive-name`
//! check derives a violation purely logically: name tokens → promise
//! class, behavior facts → observed writes/effects, contradiction →
//! finding with the full derivation. Names ending in `_mut` are exempt —
//! the suffix withdraws the read promise explicitly.

use rustmanifest_schema::TraceStep;

use super::{
    behavior::{Effect, FunctionFacts},
    insight::Insight,
    workspace::Workspace
};

/// Leading verbs that promise pure observation.
const READ_VERBS: [&str; 5] = ["get", "has", "is", "len", "peek"];

/// Effects that contradict a read promise. Observation-style effects —
/// filesystem reads, network fetches, database reads, environment
/// lookups — do not break a read promise; producing console output,
/// spawning processes, or writing the database does.
const LIE_EFFECTS: [Effect; 3] = [Effect::Process, Effect::ConsoleIo, Effect::DatabaseWrite];

/// Reports functions whose name promises a read while the body provably
/// writes state or touches the outside world.
pub(super) fn insights(workspace: &Workspace<'_>) -> Vec<Insight> {
    let mut found = Vec::new();
    for fact in workspace.facts() {
        if fact.flags.trait_impl() || fact.flags.test() {
            continue;
        }
        let Some(verb) = read_promise(fact) else {
            continue;
        };
        let writes: Vec<&str> = fact
            .writes
            .iter()
            .filter(|write| !write.starts_with("?."))
            .map(String::as_str)
            .collect();
        let effects: Vec<String> = fact
            .effects
            .iter()
            .filter(|effect| LIE_EFFECTS.contains(effect))
            .map(ToString::to_string)
            .collect();
        if writes.is_empty() && effects.is_empty() {
            continue;
        }
        let observed = if writes.is_empty() {
            format!("performs effects [{}]", effects.join(", "))
        } else if effects.is_empty() {
            format!("writes [{}]", writes.join(", "))
        } else {
            format!(
                "writes [{}] and performs effects [{}]",
                writes.join(", "),
                effects.join(", ")
            )
        };
        let detail = format!(
            "fn `{}` is named as a read (`{verb}`) but {observed}; rename it to state what it \
             does, or remove the side effects",
            fact.function
        );
        let trace = vec![
            TraceStep {
                fact:   format!("name of `{}` starts with read-verb `{verb}`", fact.function),
                reason: "the verb taxonomy classes it as a pure observation promise".to_owned()
            },
            TraceStep {
                fact:   format!("body of `{}` {observed}", fact.function),
                reason: "observed in the behavioral facts of the body".to_owned()
            },
            TraceStep {
                fact:   "the promise and the observation contradict".to_owned(),
                reason: "a read must not write state, print, or spawn processes".to_owned()
            },
        ];
        found.push(Insight::new(fact.source, fact.range, detail, trace));
    }
    found
}

/// Returns the read-verb the function's bare name leads with, unless the
/// `_mut` suffix withdraws the promise.
fn read_promise(fact: &FunctionFacts) -> Option<&'static str> {
    let bare = fact
        .function
        .rsplit("::")
        .next()
        .unwrap_or(fact.function.as_str());
    if bare.ends_with("_mut") {
        return None;
    }
    READ_VERBS.iter().copied().find(|verb| {
        bare == *verb
            || bare
                .strip_prefix(verb)
                .is_some_and(|rest| rest.starts_with('_'))
    })
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
    use crate::{
        semantic::{settling::Settling, workspace::Workspace},
        source::Source
    };

    fn found(text: &str) -> usize {
        let sources = [Source::new(PathBuf::from("src/lib.rs"), text.to_owned())];
        insights(&Workspace::resolved(&sources, &crate::semantic::settling::compiler_over(&sources))).len()
    }

    /// The findings over `text`, read with what `settle` states a
    /// resolver settled about it — a write is judged only where the
    /// resolver named the type it lands on.
    fn found_settled<'text>(
        text: &'text str,
        settle: impl FnOnce(Settling<'text>) -> Settling<'text>
    ) -> usize {
        let sources = [Source::new(PathBuf::from("src/lib.rs"), text.to_owned())];
        let resolution = settle(Settling::over("src/lib.rs", text)).done();
        insights(&Workspace::resolved(&sources, &resolution)).len()
    }

    #[test]
    fn getter_that_writes_the_database_is_a_lie_but_selecting_is_not() {
        assert_eq!(
            found("fn get_user() { query!(\"INSERT INTO audit (a) VALUES (1)\"); }"),
            1
        );
        assert_eq!(
            found("fn get_user() { let row = query!(\"SELECT id FROM users\"); }"),
            0
        );
    }

    #[test]
    fn getter_that_writes_is_a_lie() {
        assert_eq!(
            found_settled(
                "struct Counter { hits: u64 }\nimpl Counter { fn get_hits(&mut self) -> u64 { \
                 self.hits += 1; self.hits } }",
                |settling| settling.field("self.hits", "Counter", "u64")
            ),
            1
        );
    }

    #[test]
    fn getter_that_prints_is_a_lie() {
        assert_eq!(
            found("fn get_name(name: &str) -> String { println!(\"{name}\"); name.to_owned() }"),
            1
        );
    }

    #[test]
    fn honest_getter_passes() {
        assert_eq!(
            found(
                "struct Counter { hits: u64 }\nimpl Counter { fn get_hits(&self) -> u64 { \
                 self.hits } }"
            ),
            0
        );
    }

    #[test]
    fn mut_suffix_withdraws_the_promise() {
        assert_eq!(
            found(
                "struct Counter { hits: u64 }\nimpl Counter { fn get_hits_mut(&mut self) -> &mut \
                 u64 { self.hits += 0; &mut self.hits } }"
            ),
            0
        );
    }

    #[test]
    fn non_read_names_are_not_judged() {
        assert_eq!(
            found(
                "struct Counter { hits: u64 }\nimpl Counter { fn update(&mut self) { self.hits \
                 += 1; } }"
            ),
            0
        );
    }

    #[test]
    fn prefix_must_be_word_aligned() {
        assert_eq!(
            found("fn generate(seed: u64) -> u64 { println!(\"{seed}\"); seed }"),
            0
        );
    }

    #[test]
    fn trace_carries_the_derivation() {
        let text = "struct Counter { hits: u64 }\nimpl Counter { fn get_hits(&mut self) -> u64 { \
                    self.hits += 1; self.hits } }";
        let sources = [Source::new(PathBuf::from("src/lib.rs"), text.to_owned())];
        let resolution = Settling::over("src/lib.rs", text)
            .field("self.hits", "Counter", "u64")
            .done();
        let mut resolution = resolution;
        crate::semantic::settling::settle_declared(&mut resolution, &sources);
        crate::semantic::settling::settle_standard_macros(&mut resolution, &sources);
        let insights = insights(&Workspace::resolved(&sources, &resolution));
        assert_eq!(insights[0].trace.len(), 3);
        assert!(insights[0].trace[0].fact.contains("read-verb `get`"));
        assert!(insights[0].trace[1].fact.contains("writes [Counter.hits]"));
    }
}
