// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The question layer's ownership form: what a written call does with
//! each value the caller hands it.
//!
//! The answer reads the callee's declared parameter list — facts already
//! recorded for every workspace function — and states, one step per
//! parameter, whether the value moves in, crosses as a copy, or is only
//! borrowed for the call. A move or copy is claimed only where
//! `Copy`-ness is decidable: the std tables answer for well-known
//! library types, the type graph answers for workspace types by their
//! derives and impls, and everything else is stated as a by-value pass
//! whose move-or-copy the facts cannot settle — never guessed either
//! way. A call of an external or unresolved callee, or one declaring no
//! parameters, has no story and answers with nothing.

mod marks;

use rustmanifest_schema::TraceStep;

pub use self::marks::{Mark, handover_marks};
use super::{
    behavior::{CallSite, FunctionFacts, ParamFacts, ParamMode},
    calls::CallGraph,
    graph::TypeGraph,
    std_knowledge,
    why::Answer
};

/// Answers what the written call covering `offset` in source `file`
/// does with ownership, or nothing when no recorded call of a resolved
/// workspace callee with parameters covers the position.
#[must_use]
pub fn handover_at(
    facts: &[FunctionFacts],
    graph: &CallGraph,
    types: &TypeGraph,
    settled: Option<&rustmanifest_schema::Resolution>,
    file: usize,
    offset: u32
) -> Option<Answer> {
    let (asking, site) = covering_site(facts, file, offset)?;
    let callee = &facts[resolved_callee(facts, graph, asking, site)?];
    if callee.params.is_empty() {
        return None;
    }
    Some(story(callee, types, settled))
}

/// The innermost recorded call site of `file` covering `offset`, with
/// the fact index of the function it stands in.
fn covering_site(facts: &[FunctionFacts], file: usize, offset: u32) -> Option<(usize, &CallSite)> {
    facts
        .iter()
        .enumerate()
        .filter(|(_, fact)| fact.source == file)
        .flat_map(|(index, fact)| fact.call_sites.iter().map(move |site| (index, site)))
        .filter(|(_, site)| (site.site.start..site.site.end).contains(&offset))
        .min_by_key(|(_, site)| site.site.end - site.site.start)
}

/// The one resolved callee of `caller` matching the site's candidate
/// name, or nothing where none or several match.
fn resolved_callee(
    facts: &[FunctionFacts],
    graph: &CallGraph,
    caller: usize,
    site: &CallSite
) -> Option<usize> {
    let short = site
        .candidate
        .rsplit("::")
        .next()
        .unwrap_or(&site.candidate);
    let matched: Vec<usize> = graph
        .callees(caller)
        .iter()
        .copied()
        .filter(|&index| {
            facts[index].function == site.candidate
                || facts[index].function.rsplit("::").next() == Some(short)
        })
        .collect();
    let [found] = matched[..] else {
        return None;
    };
    Some(found)
}

/// The ownership story of one call: a step per declared parameter and a
/// conclusion counting where the values go.
fn story(
    callee: &FunctionFacts,
    types: &TypeGraph,
    settled: Option<&rustmanifest_schema::Resolution>
) -> Answer {
    let mut steps = Vec::new();
    let mut tallies = [0u32; 4];
    for param in &callee.params {
        let (step, tally) = told(param, types, settled);
        steps.push(step);
        tallies[tally] += 1;
    }
    let told: Vec<String> = [
        (tallies[0], format!("moves {} value(s) in", tallies[0])),
        (tallies[1], format!("copies {} value(s)", tallies[1])),
        (tallies[2], format!("borrows {} value(s)", tallies[2])),
        (
            tallies[3],
            format!(
                "takes {} value(s) whose move-or-copy is unsettled",
                tallies[3]
            )
        )
    ]
    .into_iter()
    .filter(|(count, _)| *count > 0)
    .map(|(_, text)| text)
    .collect();
    Answer {
        refused: false,
        conclusion: format!("calling `{}` {}", callee.function, told.join(", ")),
        steps
    }
}

/// One parameter's step, with the tally slot it lands in: move, copy,
/// borrow, or unsettled by-value pass.
fn told(
    param: &ParamFacts,
    types: &TypeGraph,
    settled: Option<&rustmanifest_schema::Resolution>
) -> (TraceStep, usize) {
    let (name, type_name) = (&param.name, &param.type_name);
    match param.mode {
        ParamMode::Owned => match copyness(types, settled, type_name) {
            Some(false) => (
                TraceStep {
                    fact:   format!(
                        "`{name}: {type_name}` is taken by value, and `{type_name}` is not `Copy`"
                    ),
                    reason: "the value moves: the parameter owns it now, and the caller's name \
                             for it is unusable after this line"
                        .to_owned()
                },
                0
            ),
            Some(true) => (
                TraceStep {
                    fact:   format!(
                        "`{name}: {type_name}` is taken by value, and `{type_name}` is `Copy`"
                    ),
                    reason: "a copy crosses: the caller keeps its own value untouched".to_owned()
                },
                1
            ),
            None => (
                TraceStep {
                    fact:   format!("`{name}: {type_name}` is taken by value"),
                    reason: "move or copy depends on `Copy`, which is not provable from the \
                             workspace and the std tables"
                        .to_owned()
                },
                3
            )
        },
        ParamMode::Borrowed if type_name == "str" => (
            TraceStep {
                fact:   format!("`{name}: &str` borrows a string slice"),
                reason: "no ownership is asked for: any string data satisfies it — a literal, a \
                         `String`, or a piece of one"
                    .to_owned()
            },
            2
        ),
        ParamMode::Borrowed => (
            TraceStep {
                fact:   format!("`{name}: &{type_name}` is a shared borrow"),
                reason: "the callee may only read through it; the owner keeps the value"
                    .to_owned()
            },
            2
        ),
        ParamMode::BorrowedMut => (
            TraceStep {
                fact:   format!("`{name}: &mut {type_name}` is an exclusive borrow"),
                reason: "the callee may write through it; the owner keeps the value, and no \
                         other access lives while the call runs"
                    .to_owned()
            },
            2
        )
    }
}

/// Whether `type_name` is `Copy`, where that is provable.
///
/// The language's own primitives answer for themselves, the type graph
/// answers for workspace types by their derives and impls, and nothing
/// answers otherwise.
///
/// A library type is the compiler's answer, carried per type by a
/// resolution that ran. Where none ran the question is unanswered rather
/// than guessed: two written lists of library names used to answer it, and
/// both were the compiler's answer transcribed by hand — incomplete by
/// construction and silently wrong for every type nobody thought to add.
#[must_use]
pub fn copyness(
    types: &TypeGraph,
    settled: Option<&rustmanifest_schema::Resolution>,
    type_name: &str
) -> Option<bool> {
    if std_knowledge::is_primitive_copy(type_name) {
        return Some(true);
    }
    if let Some(granted) = granted_copy(settled, type_name) {
        return Some(granted);
    }
    if types.ability_given(type_name, "Copy") {
        return Some(true);
    }
    types
        .type_named(type_name)
        .map(|_| false)
}

/// What a resolver settled about values of the type named `named`, where
/// one ran and mentioned that type at all.
fn granted_copy(settled: Option<&rustmanifest_schema::Resolution>, named: &str) -> Option<bool> {
    settled?
        .types_granted
        .iter()
        .find(|row| {
            crate::semantic::behavior::outermost_name(&row.named).as_deref() == Some(named)
        })
        .map(|row| row.copy)
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

    use super::handover_at;
    use crate::{
        semantic::{
            calls::{call_graph, resolution_context_with},
            graph::TypeGraph,
            settling::settle_identity,
            why::Answer
        },
        source::Source
    };

    fn asked(text: &str, at: &str) -> Option<Answer> {
        asked_settled(text, at, &[])
    }

    /// The same question with what a resolver settled about the types
    /// named in `granted` — each a type name and whether the compiler
    /// says values of it are copied — and about which body each written
    /// call reaches.
    fn asked_settled(text: &str, at: &str, granted: &[(&str, bool)]) -> Option<Answer> {
        let sources = [Source::new(PathBuf::from("src/lib.rs"), text.to_owned())];
        let facts = crate::semantic::behavior_with(&sources, Some(&crate::semantic::settling::standard_macros_over(&sources)));
        let types = TypeGraph::from_sources(&sources);
        let offset = u32::try_from(text.find(at).unwrap()).unwrap() + 1;
        let mut resolution = rustmanifest_schema::Resolution {
            resolver: "test".to_owned(),
            types_granted: granted
                .iter()
                .map(|(named, copy)| rustmanifest_schema::ResolvedProperty {
                    named: (*named).to_owned(),
                    copy:  *copy
                })
                .collect(),
            ..rustmanifest_schema::Resolution::default()
        };
        settle_identity(&mut resolution, &sources, &facts);
        let graph = call_graph(&facts, &resolution_context_with(&sources, Some(&resolution)));
        handover_at(&facts, &graph, &types, Some(&resolution), 0, offset)
    }

    #[test]
    fn an_external_callee_has_no_story() {
        assert!(
            asked(
                "fn main() { let s = String::from(\"hello\"); s.len(); }",
                "String::from"
            )
            .is_none()
        );
    }

    #[test]
    fn a_callee_declaring_no_parameters_has_no_story() {
        assert!(asked("fn ping() {}\nfn main() { ping(); }", "ping()").is_none());
    }

}
