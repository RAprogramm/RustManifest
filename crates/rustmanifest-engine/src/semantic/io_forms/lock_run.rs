// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The run of prints that takes one stream's lock once per line.
//!
//! A run is counted per stream and only over sites standing under no
//! branch; everything the count leaves out is stated in the trace of the
//! finding it belongs to.

use rustmanifest_schema::TraceStep;

use super::{
    catalogue::{LOCK_RUN, STANDARD_STREAMS, library_knowledge, support},
    finding::{detail, trace},
    shipped_code::shipped
};
use crate::semantic::{
    behavior::{Acquisition, Grade, OutputWrite},
    insight::Insight,
    workspace::Workspace
};

/// Reports functions taking one stream's lock once per print where a
/// single acquisition would serve the whole run.
///
/// A run is counted per stream: the library gives stdout and stderr an
/// independent lock each, so a print to one and a print to the other take
/// two different locks and no single guard could serve both.
///
/// Only sites standing under no branch are counted. Such sites follow one
/// another on every path through the body, so a path reaching the last of
/// them has taken the lock once per site — which is what the finding
/// claims. A branch-guarded site is left out: the fact base records the
/// *kind* of an enclosing branch, not its identity, so two sites under a
/// branch each cannot be told apart as the same branch (both run) or two
/// exclusive ones (exactly one runs). Where a finding is reported, the
/// left-out sites are counted in its trace; where the counted run is too
/// short to report, nothing is claimed about them, which is the boundary
/// of this tier and not a judgement that they are fine.
pub(in crate::semantic) fn lock_run_insights(workspace: &Workspace<'_>) -> Vec<Insight> {
    let sources = workspace.sources();
    let library = library_knowledge();
    let mut found = Vec::new();
    for fact in workspace.facts() {
        if !shipped(sources, fact) {
            continue;
        }
        for stream in STANDARD_STREAMS {
            let Some(derived) = support(&LOCK_RUN, stream, library) else {
                continue;
            };
            let run: Vec<&OutputWrite> = fact
                .output_writes
                .iter()
                .filter(|write| write.stream == stream && counted_in_run(write))
                .collect();
            if run.len() < RUN_THRESHOLD {
                continue;
            }
            let Some(first) = run.first() else {
                continue;
            };
            let always = run
                .iter()
                .all(|write| write.guard.local_grade() == Grade::Certain);
            let observed = if always {
                format!(
                    "{} writes to {stream} on every path through the body",
                    run.len()
                )
            } else {
                format!(
                    "{} writes to {stream} on every path that reaches the last of them",
                    run.len()
                )
            };
            let uncounted = fact
                .output_writes
                .iter()
                .filter(|write| {
                    write.stream == stream
                        && matches!(write.acquisition, Acquisition::AtSite)
                        && !counted_in_run(write)
                })
                .count();
            let already_held = fact
                .output_writes
                .iter()
                .filter(|write| {
                    write.stream == stream && matches!(write.acquisition, Acquisition::Held)
                })
                .count();
            let mut steps = trace(
                &format!(
                    "{} recorded write site(s) to {stream} under no branch",
                    run.len()
                ),
                "output writes are recorded per site, in body order",
                derived
            );
            if uncounted > 0 {
                steps.push(TraceStep {
                    fact:   format!("{uncounted} further site(s) to {stream} are not counted"),
                    reason: "a branch-guarded or dead site is left out: the recorded guard names \
                             the kind of a branch, not which branch, so nothing proves two such \
                             sites share an execution"
                        .to_owned()
                });
            }
            if already_held > 0 {
                steps.push(TraceStep {
                    fact:   format!(
                        "{already_held} site(s) to {stream} write through a guard bound earlier"
                    ),
                    reason:
                        "such a site takes no lock of its own — the lock was taken once where \
                             the guard was bound, which is the form this finding asks for"
                            .to_owned()
                });
            }
            found.push(Insight::new(
                fact.source,
                first.site,
                detail(
                    &LOCK_RUN,
                    &fact.function,
                    &observed,
                    u32::try_from(run.len()).unwrap_or(u32::MAX)
                ),
                steps
            ));
        }
    }
    found
}

/// How many prints in one function make a run worth locking once.
const RUN_THRESHOLD: usize = 2;

/// Whether a write site belongs to the run its body always performs: it
/// takes the stream's lock itself, no branch stands over it, and it is
/// not proven unreachable.
///
/// A site writing through a guard bound before it is left out entirely.
/// The lock it would be charged for was taken once, at the binding, which
/// is exactly the form this check asks for — counting such sites would
/// report the fix as the defect.
const fn counted_in_run(write: &OutputWrite) -> bool {
    matches!(write.acquisition, Acquisition::AtSite)
        && write.guard.guards.is_empty()
        && !write.guard.dead
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

    use super::lock_run_insights;
    use crate::source::Source;

    fn sources(text: &str) -> Vec<Source> {
        vec![Source::new(PathBuf::from("src/main.rs"), text.to_owned())]
    }

    /// The insights of a fixture read against what a compiler settles
    /// about it: the standard macros it writes and the types it
    /// declares, without which the readings above answer nothing.
    fn insights_over(sources: &[Source]) -> Vec<crate::semantic::insight::Insight> {
        let settled = crate::semantic::settling::compiler_over(sources);
        lock_run_insights(&crate::semantic::workspace::Workspace::resolved(sources, &settled))
    }

    #[test]
    fn a_run_of_prints_is_reported_and_a_single_print_is_not() {
        let run = insights_over(&sources(
            "fn main() {\n    println!(\"one\");\n    println!(\"two\");\n}"
        ));
        assert_eq!(run.len(), 1);
        assert!(run[0].detail.contains("2 writes to stdout"));

        let single = insights_over(&sources(
            "fn main() {\n    println!(\"one\");\n}"
        ));
        assert!(single.is_empty());
    }

    /// The two standard streams hold two independent locks, so one guard
    /// could not serve both and a print to each is not a run.
    #[test]
    fn the_two_standard_streams_are_never_pooled_into_one_run() {
        let mixed = insights_over(&sources(
            "fn main() {\n    println!(\"out\");\n    eprintln!(\"err\");\n}"
        ));
        assert!(mixed.is_empty());

        let both = insights_over(&sources(
            "fn main() {\n    println!(\"a\");\n    println!(\"b\");\n    eprintln!(\"c\");\n    \
             eprintln!(\"d\");\n}"
        ));
        assert_eq!(both.len(), 2);
        assert!(both[0].detail.contains("2 writes to stdout"));
        assert!(both[1].detail.contains("2 writes to stderr"));
    }

    /// Writes standing in branches are not a run: the fact base records the
    /// kind of a branch, not its identity, so nothing proves two of them
    /// ever happen in one execution.
    #[test]
    fn writes_under_branches_are_not_counted_as_a_run() {
        let exclusive = insights_over(&sources(
            "fn main(ok: bool) {\n    if ok { println!(\"yes\"); } else { println!(\"no\"); }\n}"
        ));
        assert!(exclusive.is_empty());

        let arms = insights_over(&sources(
            "fn main(v: Command) {\n    match v {\n        Command::Place(_) => \
             println!(\"placing\"),\n        Command::Cancel(_) => println!(\"cancelling\")\n    \
             }\n}"
        ));
        assert!(arms.is_empty());

        let dead = insights_over(&sources(
            "fn main() {\n    return;\n    println!(\"a\");\n    println!(\"b\");\n}"
        ));
        assert!(dead.is_empty());
    }

    /// A branch-guarded site left out of the count is still stated, so the
    /// reader knows the run reported is not the whole story.
    #[test]
    fn sites_left_out_of_a_run_are_stated_in_the_trace() {
        let mixed = insights_over(&sources(
            "fn main(ok: bool) {\n    println!(\"a\");\n    println!(\"b\");\n    if ok { \
             println!(\"c\"); }\n}"
        ));
        assert_eq!(mixed.len(), 1);
        assert!(mixed[0].detail.contains("2 writes to stdout"));
        assert!(mixed[0].trace.iter().any(|step| {
            step.fact
                .contains("1 further site(s) to stdout are not counted")
        }));
    }

    /// A run of writes through one guard bound before them is the form
    /// this finding asks for, so it is never reported as a deviation from
    /// itself: the lock was taken once, at the binding.
    #[test]
    fn writes_through_a_guard_bound_once_are_not_a_run() {
        let locked_once = insights_over(&sources(
            "fn main() -> std::io::Result<()> {\n    let mut out = std::io::stdout().lock();\n    \
             out.write_all(b\"one\")?;\n    out.write_all(b\"two\")?;\n    \
             out.write_all(b\"three\")\n}"
        ));
        assert!(locked_once.is_empty());
    }

    /// A site standing after an exit still belongs to the run: reaching it
    /// means every unguarded site before it has already written.
    #[test]
    fn a_run_after_an_early_exit_claims_only_the_paths_that_reach_it() {
        let weakened = insights_over(&sources(
            "fn main() -> Result<(), ()> {\n    println!(\"a\");\n    helper()?;\n    \
             println!(\"b\");\n    Ok(())\n}"
        ));
        assert_eq!(weakened.len(), 1);
        assert!(
            weakened[0]
                .detail
                .contains("2 writes to stdout on every path that reaches the last of them")
        );
    }
}
