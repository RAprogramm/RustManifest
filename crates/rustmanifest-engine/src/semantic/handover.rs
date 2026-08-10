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

use rustmanifest_schema::TraceStep;

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
    file: usize,
    offset: u32
) -> Option<Answer> {
    let (asking, site) = covering_site(facts, file, offset)?;
    let callee = &facts[resolved_callee(facts, graph, asking, site)?];
    if callee.params.is_empty() {
        return None;
    }
    Some(story(callee, types))
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
fn story(callee: &FunctionFacts, types: &TypeGraph) -> Answer {
    let mut steps = Vec::new();
    let mut tallies = [0u32; 4];
    for param in &callee.params {
        let (step, tally) = told(param, types);
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
        conclusion: format!("calling `{}` {}", callee.function, told.join(", ")),
        steps
    }
}

/// One parameter's step, with the tally slot it lands in: move, copy,
/// borrow, or unsettled by-value pass.
fn told(param: &ParamFacts, types: &TypeGraph) -> (TraceStep, usize) {
    let (name, type_name) = (&param.name, &param.type_name);
    match param.mode {
        ParamMode::Owned => match copyness(types, type_name) {
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

/// Whether `type_name` is `Copy`, where that is provable: the std
/// tables answer for known library types, the type graph for workspace
/// types by their derives and impls, and nothing answers otherwise.
pub(super) fn copyness(types: &TypeGraph, type_name: &str) -> Option<bool> {
    if std_knowledge::is_copy_type(type_name) {
        return Some(true);
    }
    if std_knowledge::is_owning_type(type_name) {
        return Some(false);
    }
    let node = types.type_named(type_name)?;
    Some(
        node.derives.iter().any(|derive| derive == "Copy")
            || types.traits_implemented(type_name).contains(&"Copy")
    )
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
            behavior::behavior,
            calls::{call_graph, resolution_context},
            graph::TypeGraph,
            why::Answer
        },
        source::Source
    };

    fn asked(text: &str, at: &str) -> Option<Answer> {
        let sources = [Source::new(PathBuf::from("src/lib.rs"), text.to_owned())];
        let facts = behavior(&sources);
        let graph = call_graph(&facts, &resolution_context(&sources));
        let types = TypeGraph::from_sources(&sources);
        let offset = u32::try_from(text.find(at).unwrap()).unwrap() + 1;
        handover_at(&facts, &graph, &types, 0, offset)
    }

    #[test]
    fn a_by_value_pass_of_a_known_owner_is_told_as_a_move() {
        let answer = asked(
            "fn foo(value: String) {}\nfn main() { let s = String::from(\"hello\"); foo(s); }",
            "foo(s)"
        )
        .unwrap();
        assert!(answer.steps[0].fact.contains("`String` is not `Copy`"));
        assert!(answer.steps[0].reason.contains("unusable after this line"));
        assert!(answer.conclusion.contains("moves 1 value(s) in"));
    }

    #[test]
    fn a_shared_borrow_is_told_as_the_owner_keeping_the_value() {
        let answer = asked(
            "fn foo(value: &String) {}\nfn main() { let s = String::from(\"hello\"); foo(&s); }",
            "foo(&s)"
        )
        .unwrap();
        assert!(answer.steps[0].fact.contains("shared borrow"));
        assert!(answer.steps[0].reason.contains("owner keeps the value"));
        assert!(answer.conclusion.contains("borrows 1 value(s)"));
    }

    #[test]
    fn a_slice_parameter_is_told_as_wanting_no_ownership() {
        let answer = asked(
            "fn foo(value: &str) {}\nfn main() { let s = String::from(\"hello\"); foo(&s); }",
            "foo(&s)"
        )
        .unwrap();
        assert!(answer.steps[0].fact.contains("borrows a string slice"));
        assert!(answer.steps[0].reason.contains("no ownership is asked for"));
    }

    #[test]
    fn an_exclusive_borrow_is_told_as_writable_and_exclusive() {
        let answer = asked(
            "fn foo(value: &mut String) {}\nfn main() { let mut s = String::new(); foo(&mut s); \
             }",
            "foo(&mut s)"
        )
        .unwrap();
        assert!(answer.steps[0].fact.contains("exclusive borrow"));
        assert!(answer.steps[0].reason.contains("may write"));
    }

    #[test]
    fn a_copy_type_by_value_is_told_as_a_copy() {
        let answer = asked("fn foo(value: u32) {}\nfn main() { foo(7); }", "foo(7)").unwrap();
        assert!(answer.steps[0].fact.contains("`u32` is `Copy`"));
        assert!(answer.conclusion.contains("copies 1 value(s)"));
    }

    #[test]
    fn a_workspace_type_without_copy_is_told_as_a_move() {
        let answer = asked(
            "struct Token { text: String }\nfn eat(token: Token) {}\nfn main() { eat(Token { \
             text: String::new() }); }",
            "eat(Token"
        )
        .unwrap();
        assert!(answer.steps[0].fact.contains("`Token` is not `Copy`"));
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

    #[test]
    fn the_innermost_covered_call_answers() {
        let answer = asked(
            "fn inner(value: String) -> u32 { value.len() as u32 }\nfn outer(count: u32) {}\nfn \
             main() { outer(inner(String::new())); }",
            "inner(String::new()"
        )
        .unwrap();
        assert!(answer.conclusion.contains("calling `inner`"));
    }
}
