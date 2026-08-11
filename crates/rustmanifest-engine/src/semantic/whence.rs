// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The question layer's second form: where does the value a function
//! hands back come from?
//!
//! The answer walks the body's own handback records — one step per
//! written position, citing the channel that recorded it and the
//! position's byte range — and then restates each origin across calls
//! through the summary composition, whose only free variables are the
//! function's own inputs. The same honesty rules as the first form: a
//! question the facts cannot answer gets a refusal that names what was
//! looked for — and names the candidates with their files, so an
//! ambiguous asker can ask again more precisely.

use rustmanifest_schema::TraceStep;

use super::{
    behavior::{FunctionFacts, Provenance},
    calls::CallGraph,
    meaning::{origin_text, qualifiers_by_fact},
    summary::composition_with,
    why::Answer,
    workspace::Workspace
};
use crate::source::Source;

/// Answers where the value `function` hands back comes from.
#[must_use]
pub fn whence_value(workspace: &Workspace<'_>, function: &str) -> Answer {
    whence_value_over(
        workspace.sources(),
        workspace.facts(),
        workspace.calls(),
        function
    )
}

/// [`whence_value`] over derivations already built, so a caller holding
/// them pays for no second derivation.
#[must_use]
pub fn whence_value_over(
    sources: &[Source],
    facts: &[FunctionFacts],
    graph: &CallGraph,
    function: &str
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
        return unmatched(sources, facts, function, &named);
    };
    let fact = &facts[index];
    if fact.handbacks.is_empty() {
        return Answer {
            conclusion: format!(
                "`{}` records no position handing a value back",
                fact.function
            ),
            steps:      vec![TraceStep {
                fact:   format!("the handback channel of `{}` is empty", fact.function),
                reason: "the channel keeps one record per written position, and none is written"
                    .to_owned()
            }]
        };
    }
    let qualifiers = qualifiers_by_fact(sources, facts);
    let composed = composition_with(sources, None, facts, graph, &qualifiers);
    let mut steps = vec![TraceStep {
        fact:   format!(
            "`{}` records {} position(s) handing a value back",
            fact.function,
            fact.handbacks.len()
        ),
        reason: "the handback channel keeps one record per written position".to_owned()
    }];
    let mut spoken = Vec::new();
    for (position, handback) in fact.handbacks.iter().enumerate() {
        steps.push(TraceStep {
            fact:   format!(
                "position {position} at bytes {}..{} draws from {}",
                handback.site.start,
                handback.site.end,
                cited(fact, &handback.value)
            ),
            reason: "the body's own record names the source of the value".to_owned()
        });
        let text = origin_text(&composed.of(index, &handback.value));
        steps.push(TraceStep {
            fact:   format!("restated across calls, position {position} hands back {text}"),
            reason: "a summary's only free variables are the function's own inputs, so every \
                     call the value passes through is restated"
                .to_owned()
        });
        spoken.push(text);
    }
    spoken.dedup();
    Answer {
        conclusion: format!("`{}` hands back {}", fact.function, spoken.join("; ")),
        steps
    }
}

/// The in-body citation of one origin: the channel that recorded it and
/// the position it stands at, before any call is restated.
fn cited(fact: &FunctionFacts, origin: &Provenance) -> String {
    match origin {
        Provenance::Written(literal) => format!("the written value {}", literal.quoted()),
        Provenance::Parameter(slot) => slot.name.as_ref().map_or_else(
            || format!("the input at position {}", slot.position),
            |name| format!("the input `{name}`")
        ),
        Provenance::Read(position) => {
            format!("the read at position {position} of the input channel")
        }
        Provenance::Computed(position) => {
            format!("the operation at position {position} of the operation channel")
        }
        Provenance::Chosen(position) => {
            format!("the choice at position {position} of the choice channel")
        }
        Provenance::Called(position) => fact.call_sites.get(*position as usize).map_or_else(
            || format!("the call at position {position} of the call channel"),
            |site| format!("the value `{}` hands back", site.candidate)
        ),
        Provenance::Field(key) => format!("the field `{key}`"),
        _ => "a source the restated term names itself".to_owned()
    }
}

/// The refusal for a name the facts cannot answer for: what was looked
/// for, and every candidate with its file — an ambiguous asker can ask
/// again more precisely.
fn unmatched(
    sources: &[Source],
    facts: &[FunctionFacts],
    function: &str,
    named: &[usize]
) -> Answer {
    if named.is_empty() {
        return Answer {
            conclusion: format!("no function named `{function}` is recorded"),
            steps:      Vec::new()
        };
    }
    let listed: Vec<String> = named
        .iter()
        .filter_map(|&index| {
            let fact = facts.get(index)?;
            let file = sources
                .get(fact.source)
                .map_or_else(String::new, |source| source.path().display().to_string());
            Some(format!("`{}` in {file}", fact.function))
        })
        .collect();
    Answer {
        conclusion: format!(
            "`{function}` is ambiguous: {} candidates — {}",
            named.len(),
            listed.join(", ")
        ),
        steps:      Vec::new()
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

    use super::whence_value;
    use crate::semantic::workspace::Workspace;

    fn one(text: &str) -> Vec<crate::source::Source> {
        vec![crate::source::Source::new(
            PathBuf::from("src/lib.rs"),
            text.to_owned()
        )]
    }

    /// A written literal answers as itself, with the position's byte
    /// range cited on the way.
    #[test]
    fn a_written_value_answers_as_itself() {
        let sources = one("pub fn make() -> u32 {\n    7\n}\n");
        let held = whence_value(&Workspace::new(&sources), "make");
        assert!(held.conclusion.contains("the written value"), "{held:?}");
        assert!(
            held.steps.iter().any(|step| step.fact.contains("bytes")),
            "{held:?}"
        );
    }

    /// A value drawn through a call is restated in the callee's own
    /// terms: the caller's answer names the callee's written value.
    #[test]
    fn a_called_value_is_restated_across_the_call() {
        let sources =
            one("fn seed() -> u32 {\n    7\n}\n\npub fn make() -> u32 {\n    seed()\n}\n");
        let held = whence_value(&Workspace::new(&sources), "make");
        assert!(held.conclusion.contains("the written value"), "{held:?}");
        assert!(
            held.steps
                .iter()
                .any(|step| step.fact.contains("`seed` hands back")),
            "{held:?}"
        );
        assert!(
            held.steps
                .iter()
                .any(|step| step.fact.contains("restated across calls")),
            "{held:?}"
        );
    }

    /// An input stays the term's free variable: the answer names the
    /// parameter instead of guessing a caller.
    #[test]
    fn an_input_stays_the_free_variable() {
        let sources = one("pub fn keep(count: u32) -> u32 {\n    count\n}\n");
        let held = whence_value(&Workspace::new(&sources), "keep");
        assert!(held.conclusion.contains("`count`"), "{held:?}");
    }

    /// An ambiguous name is refused with every candidate's file, so the
    /// asker can ask again more precisely.
    #[test]
    fn an_ambiguous_name_is_refused_with_files() {
        let sources = one(
            "pub struct A;\nimpl A {\n    pub fn make() -> u32 { 1 }\n}\npub struct B;\nimpl B \
             {\n    pub fn make() -> u32 { 2 }\n}\n"
        );
        let held = whence_value(&Workspace::new(&sources), "make");
        assert!(held.conclusion.contains("ambiguous"), "{held:?}");
        assert!(held.conclusion.contains("src/lib.rs"), "{held:?}");
        assert!(held.conclusion.contains("`A::make`"), "{held:?}");
    }

    /// A function with no handback answers honestly: the channel is
    /// empty, and the answer says so instead of inventing a value.
    #[test]
    fn a_function_handing_nothing_back_says_so() {
        let sources = one("pub fn run() {\n    let _held = 7;\n}\n");
        let held = whence_value(&Workspace::new(&sources), "run");
        assert!(
            held.conclusion.contains("no position handing a value back"),
            "{held:?}"
        );
    }
}
