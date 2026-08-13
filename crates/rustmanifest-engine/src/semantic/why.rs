// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The question layer's first form: why does a function carry an effect?
//!
//! An answer is an inference over recorded facts, one step at a time,
//! and nothing else: each step is a fact with the reason it is believed,
//! the last step cites the written path that proved the effect, and a
//! hop established by dispatch says on its own step that the effect is
//! possible rather than certain. A question the facts cannot answer gets
//! a refusal that names what was looked for — never a guess, and never
//! an answer about a different function than the one asked about.

use rustmanifest_schema::TraceStep;

use super::{
    behavior::FunctionFacts,
    calls::CallGraph,
    invariant::chain::{edge_reason, effect_chain},
    workspace::Workspace
};

/// One answered question: the conclusion and the steps that carry it.
///
/// The steps are the answer; the conclusion only restates their end. An
/// empty step list means the question itself failed — the conclusion
/// then says on what.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    /// What the steps add up to, in one sentence.
    pub conclusion: String,
    /// The inference, one recorded fact per step, with its reason.
    pub steps:      Vec<TraceStep>,
    /// Whether the question itself was refused — an unknown or
    /// ambiguous subject — rather than answered about the asked one.
    /// A script tells the two apart by this field, never by prose.
    pub refused:    bool
}

/// Answers why `function` carries `effect`, from the recorded facts.
///
/// Four honest outcomes: the function is unknown or ambiguous by the
/// asked name; the facts prove no such effect for it; the body performs
/// the effect itself; or a resolved chain of calls reaches a body that
/// does.
#[must_use]
pub fn why_effect(workspace: &Workspace<'_>, function: &str, effect: &str) -> Answer {
    why_effect_over(
        workspace.sources(),
        workspace.facts(),
        workspace.calls(),
        function,
        effect
    )
}

/// [`why_effect`] over derivations already built, so a caller holding
/// them pays for no second derivation.
#[must_use]
pub fn why_effect_over(
    sources: &[crate::source::Source],
    facts: &[FunctionFacts],
    graph: &CallGraph,
    function: &str,
    effect: &str
) -> Answer {
    let named: Vec<usize> = facts
        .iter()
        .enumerate()
        .filter(|(_, fact)| {
            fact.function == function || fact.function.rsplit("::").next() == Some(function)
        })
        .map(|(index, _)| index)
        .collect();
    let [index] = named[..] else {
        return unmatched(facts, function, &named);
    };
    let fact = &facts[index];
    if fact
        .effects
        .iter()
        .any(|observed| observed.covered_by(effect))
    {
        return direct(sources, fact, effect);
    }
    through(sources, facts, graph, index, effect)
}

/// The refusal for a name matching no function or several.
fn unmatched(facts: &[FunctionFacts], function: &str, named: &[usize]) -> Answer {
    if named.is_empty() {
        return Answer {
            refused:    true,
            conclusion: format!(
                "no analyzed function is named `{function}`, so nothing can be inferred about it"
            ),
            steps:      Vec::new()
        };
    }
    let listed: Vec<String> = named
        .iter()
        .map(|&index| format!("`{}`", facts[index].function))
        .collect();
    Answer {
        refused:    true,
        conclusion: format!(
            "`{function}` names {} functions — {} — and an answer about one of them would not \
             be an answer about the asked name",
            named.len(),
            listed.join(", ")
        ),
        steps:      Vec::new()
    }
}

/// The answer where the body performs the effect itself.
fn direct(sources: &[crate::source::Source], fact: &FunctionFacts, effect: &str) -> Answer {
    let steps = proof_steps(sources, fact, effect);
    Answer {
        refused: false,
        conclusion: format!(
            "`{}` performs `{effect}` in its own body, at the written call the steps cite",
            fact.function
        ),
        steps
    }
}

/// The answer where the effect is reached through calls.
fn through(
    sources: &[crate::source::Source],
    facts: &[FunctionFacts],
    graph: &CallGraph,
    index: usize,
    effect: &str
) -> Answer {
    let chain = effect_chain(facts, graph, index, effect);
    let Some(&sink) = chain.last() else {
        let derived: Vec<String> = facts[index]
            .effects
            .iter()
            .map(|observed| format!("`{observed}`"))
            .collect();
        return Answer {
            refused:    false,
            conclusion: format!(
                "the recorded facts prove no `{effect}` for `{}` — its own proven effects are \
                 [{}], and no resolved call chain reaches a body performing `{effect}`",
                facts[index].function,
                derived.join(", ")
            ),
            steps:      Vec::new()
        };
    };
    let mut steps = Vec::new();
    for window in chain.windows(2) {
        steps.push(TraceStep {
            fact:   format!(
                "fn `{}` calls fn `{}`",
                facts[window[0]].function, facts[window[1]].function
            ),
            reason: edge_reason(graph, window[0], window[1]).to_owned()
        });
    }
    steps.extend(proof_steps(sources, &facts[sink], effect));
    let possible = chain
        .windows(2)
        .any(|window| graph.possibilistic(window[0], window[1]));
    let certainty = if possible {
        " — one hop is a dispatch possibility, so the effect is possible rather than certain"
    } else {
        ""
    };
    Answer {
        refused: false,
        conclusion: format!(
            "`{}` reaches `{effect}` through `{}`{certainty}",
            facts[index].function, facts[sink].function
        ),
        steps
    }
}

/// The steps citing every written path that proved the effect in one
/// body, each with the file it stands in.
fn proof_steps(
    sources: &[crate::source::Source],
    fact: &FunctionFacts,
    effect: &str
) -> Vec<TraceStep> {
    fact.effect_sites
        .iter()
        .filter(|site| site.effect.covered_by(effect))
        .map(|site| {
            let file = sources
                .get(fact.source)
                .map_or_else(String::new, |source| source.path().display().to_string());
            TraceStep {
                fact:   format!(
                    "fn `{}` performs `{effect}`, proven by the written call `{}` in {file}",
                    fact.function, site.proof
                ),
                reason: "effect-sink call observed in the body".to_owned()
            }
        })
        .collect()
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

    use super::why_effect;
    use crate::{
        semantic::{settling::identity_over, workspace::Workspace},
        source::Source
    };

    const LAYERED: &str = "fn save(path: &str) { std::fs::write(path, \"x\").ok(); }\nfn \
                           persist() { save(\"data\"); }\nfn run() { persist(); }\n";

    fn workspace_of(text: &str) -> Vec<Source> {
        vec![Source::new(PathBuf::from("src/lib.rs"), text.to_owned())]
    }

    #[test]
    fn a_transitive_effect_is_answered_step_by_step_down_to_the_proof() {
        let sources = workspace_of(LAYERED);
        let settled = identity_over(&sources);
        let workspace = Workspace::resolved(&sources, &settled);
        let answer = why_effect(&workspace, "run", "fs");
        assert!(
            answer.conclusion.contains("reaches `fs` through `save`"),
            "{answer:?}"
        );
        assert_eq!(answer.steps.len(), 3, "{answer:?}");
        assert!(answer.steps[0].fact.contains("`run` calls fn `persist`"));
        assert!(answer.steps[1].fact.contains("`persist` calls fn `save`"));
        assert!(
            answer.steps[2]
                .fact
                .contains("proven by the written call `std::fs::write`"),
            "{answer:?}"
        );
        assert!(answer.steps[2].fact.contains("src/lib.rs"));
    }

    #[test]
    fn an_effect_the_facts_do_not_prove_is_refused_with_what_was_derived() {
        let sources = workspace_of(LAYERED);
        let settled = crate::semantic::settling::settled_over(&sources);
        let workspace = Workspace::resolved(&sources, &settled);
        let answer = why_effect(&workspace, "run", "net");
        assert!(answer.steps.is_empty());
        assert!(
            answer.conclusion.contains("prove no `net` for `run`"),
            "{answer:?}"
        );
    }

    #[test]
    fn an_unknown_or_ambiguous_name_is_refused_by_name() {
        let sources = workspace_of(
            "struct A;\nimpl A { fn go(&self) {} }\nstruct B;\nimpl B { fn go(&self) {} }\n"
        );
        let settled = crate::semantic::settling::settled_over(&sources);
        let workspace = Workspace::resolved(&sources, &settled);
        let missing = why_effect(&workspace, "vanish", "fs");
        assert!(missing.conclusion.contains("no analyzed function is named"));
        let split = why_effect(&workspace, "go", "fs");
        assert!(split.conclusion.contains("names 2 functions"), "{split:?}");
    }

    #[test]
    fn a_direct_effect_cites_its_own_written_call() {
        let sources = workspace_of(LAYERED);
        let settled = crate::semantic::settling::settled_over(&sources);
        let workspace = Workspace::resolved(&sources, &settled);
        let answer = why_effect(&workspace, "save", "fs");
        assert!(answer.conclusion.contains("in its own body"), "{answer:?}");
        assert_eq!(answer.steps.len(), 1, "{answer:?}");
    }
}
