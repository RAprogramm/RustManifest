// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Loop-body ownership checks: `loop-invariant-clone` and
//! `loop-allocation`.
//!
//! `loop-invariant-clone` reports a `let` directly inside a loop body that
//! clones a place whose root is neither bound nor mutated inside the loop,
//! when the copy itself is only ever read — one clone before the loop (or a
//! borrow) would do.
//!
//! `loop-allocation` reports a `let` directly inside a loop body that
//! allocates a fresh container (`Vec::new`, `String::new`,
//! `with_capacity`, empty `vec![]`) which is used during the iteration but
//! never leaves it — hoisting the buffer and clearing it per iteration
//! avoids one heap allocation per pass.
//!
//! Both checks are conservative on the same fronts: only `let` statements
//! that are direct children of the loop block are considered; any
//! unclassified bare use of the binding (a move, a return, a call
//! argument) disqualifies it; any mention of the binding inside a macro
//! invocation disqualifies it, because macros can move or mutate what they
//! mention; and a clone receiver mentioned in any macro or mutated
//! anywhere in the loop is treated as unstable.
//!
//! The module splits along responsibilities: [`collect`] finds candidate
//! bindings, [`origin`] classifies how a binding got its value, [`uses`]
//! classifies how it is used, and this file holds only the two rule
//! decisions. A new loop rule is a new decision function here plus, at
//! most, a new origin variant.

mod collect;
mod origin;
mod uses;

use rustmanifest_schema::{Applicability, Fix, Range, TraceStep};

use self::{collect::candidates, origin::Origin};
use super::{insight::Insight, workspace::Workspace};

/// Reports loop-invariant clones that could be hoisted out of the loop.
pub(super) fn clone_insights(workspace: &Workspace<'_>) -> Vec<Insight> {
    let sources = workspace.sources();
    let mut found = Vec::new();
    for candidate in candidates(sources, workspace.resolution()) {
        let Origin::Clone {
            ref chain,
            ref method,
            stable
        } = candidate.origin
        else {
            continue;
        };
        let uses = &candidate.uses;
        if !stable
            || uses.moves > 0
            || uses.mutations > 0
            || uses.mut_borrows > 0
            || uses.calls > 0
            || uses.unsettled > 0
            || uses.bare > 0
            || uses.macro_mentions > 0
            || uses.field_reads + uses.shared_borrows == 0
        {
            continue;
        }
        let name = &candidate.name;
        let detail = format!(
            "fn `{}` clones `{chain}` into `{name}` on every loop iteration, but `{name}` is \
             only read and `{chain}` never changes inside the loop; clone once before the loop \
             or borrow instead",
            candidate.function
        );
        let trace = vec![
            TraceStep {
                fact:   format!("`{name}` is initialized from `{chain}.{method}()` inside a loop"),
                reason: "the `let` sits directly in a `for`/`while`/`loop` block".to_owned()
            },
            TraceStep {
                fact:   format!("`{chain}` is neither rebound nor mutated inside the loop"),
                reason: "no assignment, `&mut` borrow, mutating call, or macro targets its root"
                    .to_owned()
            },
            TraceStep {
                fact:   format!("every observed use of `{name}` is a read"),
                reason: "only field reads and shared borrows were seen after the `let`".to_owned()
            },
        ];
        let fixes = candidate.hoist.as_ref().map_or_else(Vec::new, |plan| {
            vec![
                Fix {
                    file:          None,
                    rename_to:     None,
                    range:         plan.stmt_lines,
                    replacement:   String::new(),
                    applicability: Applicability::MachineApplicable
                },
                Fix {
                    file:          None,
                    rename_to:     None,
                    range:         Range {
                        start: plan.insert_at,
                        end:   plan.insert_at
                    },
                    replacement:   format!("{}{}\n", plan.outer_indent, plan.stmt_text),
                    applicability: Applicability::MachineApplicable
                },
            ]
        });
        found.push(
            Insight::new(candidate.source, candidate.range, detail, trace).with_fixes(fixes)
        );
    }
    found
}

/// Reports per-iteration container allocations that never escape the
/// iteration.
pub(super) fn allocation_insights(workspace: &Workspace<'_>) -> Vec<Insight> {
    let sources = workspace.sources();
    let mut found = Vec::new();
    for candidate in candidates(sources, workspace.resolution()) {
        let Origin::Allocation {
            ref call
        } = candidate.origin
        else {
            continue;
        };
        let uses = &candidate.uses;
        let reads_and_writes = uses.field_reads
            + uses.shared_borrows
            + uses.mut_borrows
            + uses.mutations
            + uses.calls;
        if uses.moves > 0
            || uses.unsettled > 0
            || uses.bare > 0
            || uses.macro_mentions > 0
            || reads_and_writes == 0
        {
            continue;
        }
        let name = &candidate.name;
        let detail = format!(
            "fn `{}` allocates `{name}` with `{call}` on every loop iteration and the value \
             never leaves the iteration; hoist the buffer out of the loop and `.clear()` it \
             instead",
            candidate.function
        );
        let trace = vec![
            TraceStep {
                fact:   format!("`{name}` is initialized from `{call}` inside a loop"),
                reason: "the `let` sits directly in a `for`/`while`/`loop` block".to_owned()
            },
            TraceStep {
                fact:   format!("`{name}` never escapes the iteration"),
                reason: "it is used, but never moved, returned, or passed to a macro".to_owned()
            },
        ];
        let fixes = candidate.hoist.as_ref().map_or_else(Vec::new, |plan| {
            vec![
                Fix {
                    file:          None,
                    rename_to:     None,
                    range:         plan.stmt_lines,
                    replacement:   format!("{}{}.clear();\n", plan.inner_indent, candidate.name),
                    applicability: Applicability::MachineApplicable
                },
                Fix {
                    file:          None,
                    rename_to:     None,
                    range:         Range {
                        start: plan.insert_at,
                        end:   plan.insert_at
                    },
                    replacement:   format!("{}{}\n", plan.outer_indent, plan.stmt_text),
                    applicability: Applicability::MachineApplicable
                },
            ]
        });
        found.push(
            Insight::new(candidate.source, candidate.range, detail, trace).with_fixes(fixes)
        );
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

    use rustmanifest_schema::{Resolution, ResolvedMode};

    use super::{allocation_insights, clone_insights};
    use crate::{semantic::workspace::Workspace, source::Source};

    fn sources(text: &str) -> [Source; 1] {
        [Source::new(PathBuf::from("src/lib.rs"), text.to_owned())]
    }

    /// A resolver's answer for how each written call takes the value it
    /// stands on — the only thing that says whether a call leaves a
    /// binding alone, changes it, or ends its life.
    fn taking(text: &str, calls: &[(&str, rustmanifest_schema::ResolvedMode)]) -> Resolution {
        let mut settled = Resolution {
            resolver: "test".to_owned(),
            calls: calls
                .iter()
                .map(|(written, mode)| {
                    let start = u32::try_from(text.find(written).expect("the call is written"))
                        .expect("the fixture is small");
                    rustmanifest_schema::ResolvedCall {
                        at:       rustmanifest_schema::Location {
                            file:  PathBuf::from("src/lib.rs"),
                            range: rustmanifest_schema::Range {
                                start,
                                end: start
                                    + u32::try_from(written.len()).expect("the fixture is small")
                            }
                        },
                        callee:   written
                            .rsplit_once('.')
                            .map_or(*written, |(_, called)| called)
                            .trim_end_matches(|letter: char| !letter.is_alphanumeric())
                            .to_owned(),
                        returns:  "()".to_owned(),
                        receiver: None,
                        reaches:  None,
                        takes:    Some(*mode)
                    }
                })
                .collect(),
            ..Resolution::default()
        };
        settled.settle();
        settled
    }

    #[test]
    fn invariant_clone_read_only_in_loop_is_reported() {
        let sources = sources(
            "struct Config { retries: u32 }\nfn total(config: Config, jobs: &[u32]) -> u32 {\n    \
             let mut total = 0;\n    for job in jobs {\n        let snapshot = config.clone();\n   \
             total += snapshot.retries + job;\n    }\n    total\n}"
        );
        let settled = taking(sources[0].text(), &[("config.clone()", ResolvedMode::Borrowed)]);
        let insights = clone_insights(&Workspace::resolved(&sources, &settled));
        assert_eq!(insights.len(), 1);
        assert!(insights[0].detail.contains("`config`"));
        assert_eq!(insights[0].trace.len(), 3);
    }

    #[test]
    fn clone_of_the_loop_variable_passes() {
        let sources = sources(
            "fn copies(items: &[String]) -> u32 {\n    let mut seen = 0;\n    for item in items \
             {\n        let copy = item.clone();\n        seen += copy.len() as u32;\n    }\n    \
             seen\n}"
        );
        assert!(clone_insights(&Workspace::new(&sources)).is_empty());
    }

    #[test]
    fn mutated_clone_passes() {
        let sources = sources(
            "struct Config { retries: u32 }\nfn bump(config: Config, jobs: &[u32]) -> u32 {\n    \
             let mut last = 0;\n    for _job in jobs {\n        let mut snapshot = \
             config.clone();\n        snapshot.retries += 1;\n        last = snapshot.retries;\n  \
             }\n    last\n}"
        );
        assert!(clone_insights(&Workspace::new(&sources)).is_empty());
    }

    #[test]
    fn moved_clone_passes() {
        let sources = sources(
            "struct Config { retries: u32 }\nfn keep(config: Config, jobs: &[u32]) -> \
             Vec<Config> {\n    let mut kept = Vec::new();\n    for _job in jobs {\n        let \
             snapshot = config.clone();\n        kept.push(snapshot);\n    }\n    kept\n}"
        );
        assert!(clone_insights(&Workspace::new(&sources)).is_empty());
    }

    #[test]
    fn clone_of_a_receiver_mutated_in_the_loop_passes() {
        let sources = sources(
            "struct Config { retries: u32 }\nfn churn(mut config: Config, jobs: &[u32]) -> u32 \
             {\n    let mut total = 0;\n    for _job in jobs {\n        let snapshot = \
             config.clone();\n        total += snapshot.retries;\n        config.retries += 1;\n  \
             }\n    total\n}"
        );
        assert!(clone_insights(&Workspace::new(&sources)).is_empty());
    }

    #[test]
    fn clone_outside_a_loop_passes() {
        let sources = sources(
            "struct Config { retries: u32 }\nfn once(config: Config) -> u32 {\n    let snapshot \
             = config.clone();\n    snapshot.retries\n}"
        );
        assert!(clone_insights(&Workspace::new(&sources)).is_empty());
    }

    #[test]
    fn clone_used_in_a_macro_passes() {
        let sources = sources(
            "struct Config { retries: u32 }\nfn shout(config: Config, jobs: &[u32]) {\n    for \
             _job in jobs {\n        let snapshot = config.clone();\n        \
             println!(\"{}\", snapshot.retries);\n    }\n}"
        );
        assert!(clone_insights(&Workspace::new(&sources)).is_empty());
    }

    #[test]
    fn invariant_clone_in_a_while_loop_is_reported() {
        let sources = sources(
            "struct Config { retries: u32 }\nfn wait(config: Config) -> u32 {\n    let mut left \
             = 3;\n    while left > 0 {\n        let snapshot = config.clone();\n        left -= \
             snapshot.retries;\n    }\n    left\n}"
        );
        let settled = taking(sources[0].text(), &[("config.clone()", ResolvedMode::Borrowed)]);
        assert_eq!(
            clone_insights(&Workspace::resolved(&sources, &settled)).len(),
            1
        );
    }

    #[test]
    fn fresh_buffer_used_but_never_escaping_is_reported() {
        let sources = sources(
            "fn widths(values: &[u32]) -> usize {\n    let mut width = 0;\n    for _value in \
             values {\n        let mut line = String::with_capacity(16);\n        \
             line.push('#');\n        width += line.len();\n    }\n    width\n}"
        );
        let settled = taking(
            sources[0].text(),
            &[
                ("line.push('#')", ResolvedMode::BorrowedMut),
                ("line.len()", ResolvedMode::Borrowed),
            ]
        );
        let insights = allocation_insights(&Workspace::resolved(&sources, &settled));
        assert_eq!(insights.len(), 1);
        assert!(insights[0].detail.contains("String::with_capacity"));
        assert_eq!(insights[0].trace.len(), 2);
    }

    #[test]
    fn buffer_moved_out_of_the_iteration_passes() {
        let sources = sources(
            "fn lines(values: &[u32]) -> Vec<String> {\n    let mut lines = Vec::new();\n    for \
             _value in values {\n        let mut line = String::new();\n        \
             line.push('#');\n        lines.push(line);\n    }\n    lines\n}"
        );
        assert!(allocation_insights(&Workspace::new(&sources)).is_empty());
    }

    #[test]
    fn empty_vec_macro_in_loop_is_reported() {
        let sources = sources(
            "fn sums(values: &[u32]) -> u32 {\n    let mut total = 0;\n    for value in values \
             {\n        let mut batch = vec![];\n        batch.push(value);\n        total += \
             batch.len() as u32;\n    }\n    total\n}"
        );
        let settled = taking(
            sources[0].text(),
            &[
                ("batch.push(value)", ResolvedMode::BorrowedMut),
                ("batch.len()", ResolvedMode::Borrowed),
            ]
        );
        assert_eq!(
            allocation_insights(&Workspace::resolved(&sources, &settled)).len(),
            1
        );
    }

    #[test]
    fn unused_buffer_passes() {
        let sources = sources(
            "fn idle(values: &[u32]) {\n    for _value in values {\n        let _scratch: \
             Vec<u32> = Vec::new();\n    }\n}"
        );
        assert!(allocation_insights(&Workspace::new(&sources)).is_empty());
    }

    #[test]
    fn capacity_depending_on_the_loop_variable_passes() {
        let sources = sources(
            "fn sized(values: &[usize]) -> usize {\n    let mut width = 0;\n    for value in \
             values {\n        let line = String::with_capacity(*value);\n        width += \
             line.len();\n    }\n    width\n}"
        );
        assert!(allocation_insights(&Workspace::new(&sources)).is_empty());
    }

    #[test]
    fn buffer_consumed_by_an_into_method_passes() {
        let sources = sources(
            "fn bytes(values: &[u32]) -> usize {\n    let mut total = 0;\n    for _value in \
             values {\n        let line = String::new();\n        total += \
             line.into_bytes().len();\n    }\n    total\n}"
        );
        assert!(allocation_insights(&Workspace::new(&sources)).is_empty());
    }
}
