// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Canonical forms of writing to a standard stream, and the cost of
//! deviating from them.
//!
//! The forms live in [`FORMS`] as data: the property the code must
//! satisfy, the shape that satisfies it, what one deviation costs, and
//! the library guarantee the cost follows from. A check contributes
//! only facts — which function, which site, how many, what text — and
//! the message is assembled mechanically by [`detail`] from those facts
//! and the catalogue entry. No check writes a sentence of its own, so a
//! finding can never claim more than the catalogue and the fact base
//! together license.

use rustmanifest_schema::TraceStep;

use super::{
    behavior::{FunctionFacts, Guard, Multiplicity, Payload, Stream, behavior},
    entry::entry_points,
    insight::Insight
};
use crate::source::Source;

/// One canonical form and the cost of deviating from it.
///
/// The library guarantee behind a form is never written here: it is
/// derived from the standard library's own sources and cited by
/// property name, so a form whose property the derivation cannot find
/// produces no finding at all.
struct Form {
    /// Property name, as derived from the library sources.
    property:  &'static str,
    /// The shape that satisfies the property.
    canonical: &'static str,
    /// What one deviation costs at run time.
    cost:      &'static str
}

/// Catalogue of output forms, indexed by the check that reads it.
const LOCK_RUN: Form = Form {
    property:  "lock-per-call",
    canonical: "lock the stream handle once, then write every line through that guard",
    cost:      "one reentrant lock acquisition and release per write"
};

/// Form for writes standing inside a loop.
const LOOP_WRITE: Form = Form {
    property:  "line-buffered",
    canonical: "wrap the locked handle in a buffered writer and flush once after the loop",
    cost:      "one write syscall per iteration"
};

/// Form for constants carried through the formatting machinery.
const CONSTANT_TEXT: Form = Form {
    property:  "format-machinery",
    canonical: "write the constant's bytes directly to the locked handle",
    cost:      "one format-argument construction and piece walk per write"
};

/// Form for an entry point that cannot carry a write failure out.
const WRITE_ERROR: Form = Form {
    property:  "panic-on-failure",
    canonical: "return a result from the entry point and hand the write's result back",
    cost:      "a panic and stack unwind instead of an exit code"
};

/// Assembles the message of one finding from facts and a catalogue
/// entry, in a fixed shape no check may deviate from.
fn detail(form: &Form, subject: &str, observed: &str) -> String {
    format!(
        "`{subject}` — {observed}\n    required: {}\n    cost: {}",
        form.canonical, form.cost
    )
}

/// Assembles the trace: the observed fact, then the library property the
/// cost rests on, quoted from the library's own source with its
/// location.
fn trace(
    form: &Form,
    observed: &str,
    evidence: &str,
    library: &super::std_derive::StdOutputKnowledge
) -> Vec<TraceStep> {
    let mut steps = vec![TraceStep {
        fact:   observed.to_owned(),
        reason: evidence.to_owned()
    }];
    if let Some(derived) = library.fact(form.property) {
        steps.push(TraceStep {
            fact:   derived.observed.clone(),
            reason: format!("derived from {}:{}", derived.file.display(), derived.line)
        });
    }
    steps
}

/// Library knowledge for the active toolchain, derived once per process
/// and shared by every check, or nothing when the sources are not
/// installed.
fn library_knowledge() -> &'static super::std_derive::StdOutputKnowledge {
    static DERIVED: std::sync::OnceLock<super::std_derive::StdOutputKnowledge> =
        std::sync::OnceLock::new();
    DERIVED.get_or_init(|| {
        super::std_derive::std_source_root()
            .map(|root| super::std_derive::output_knowledge(&root))
            .unwrap_or_default()
    })
}

/// How many prints in one function make a run worth locking once.
const RUN_THRESHOLD: usize = 2;

/// Reports functions taking the stream lock once per print where one
/// acquisition would serve the whole run.
pub(super) fn lock_run_insights(sources: &[Source]) -> Vec<Insight> {
    let library = library_knowledge();
    if !library.has(LOCK_RUN.property) {
        return Vec::new();
    }
    let mut found = Vec::new();
    for fact in behavior(sources) {
        if fact.flags.test() {
            continue;
        }
        let prints: Vec<&super::behavior::OutputWrite> = fact
            .output_writes
            .iter()
            .filter(|write| standard_stream(write.stream))
            .collect();
        if prints.len() < RUN_THRESHOLD {
            continue;
        }
        let Some(first) = prints.first() else {
            continue;
        };
        found.push(Insight::new(
            fact.source,
            first.site,
            detail(
                &LOCK_RUN,
                &fact.function,
                &format!("{} writes to a standard stream in one body", prints.len())
            ),
            trace(
                &LOCK_RUN,
                &format!("{} recorded write site(s) in one body", prints.len()),
                "output writes are recorded per site, in body order",
                library
            )
        ));
    }
    found
}

/// Reports writes inside a loop, where line buffering turns every
/// iteration into a separate write syscall.
pub(super) fn loop_write_insights(sources: &[Source]) -> Vec<Insight> {
    let library = library_knowledge();
    if !library.has(LOOP_WRITE.property) {
        return Vec::new();
    }
    let mut found = Vec::new();
    for fact in behavior(sources) {
        if fact.flags.test() {
            continue;
        }
        for write in &fact.output_writes {
            if !standard_stream(write.stream) || write.multiplicity != Multiplicity::Unbounded {
                continue;
            }
            let loops = write
                .guard
                .guards
                .iter()
                .filter(|guard| matches!(guard, Guard::For | Guard::While))
                .count();
            if loops == 0 {
                continue;
            }
            found.push(Insight::new(
                fact.source,
                write.site,
                detail(
                    &LOOP_WRITE,
                    &fact.function,
                    &format!(
                        "a write to {} stands under {loops} loop construct(s)",
                        write.stream
                    )
                ),
                trace(
                    &LOOP_WRITE,
                    &format!("the write site stands under {loops} loop construct(s)"),
                    "recorded guard stack of the write site",
                    library
                )
            ));
        }
    }
    found
}

/// Reports a fully proven literal carried through the formatting
/// machinery, where a direct byte write skips it.
pub(super) fn literal_format_insights(sources: &[Source]) -> Vec<Insight> {
    let library = library_knowledge();
    if !library.has(CONSTANT_TEXT.property) {
        return Vec::new();
    }
    let mut found = Vec::new();
    for fact in behavior(sources) {
        if fact.flags.test() {
            continue;
        }
        for write in &fact.output_writes {
            if !standard_stream(write.stream) {
                continue;
            }
            let Payload::Literal(text) = &write.payload else {
                continue;
            };
            if text.is_empty() {
                continue;
            }
            found.push(Insight::new(
                fact.source,
                write.site,
                detail(
                    &CONSTANT_TEXT,
                    &fact.function,
                    &format!("the written text is fully proven: `{text}`")
                ),
                trace(
                    &CONSTANT_TEXT,
                    &format!("the written text is fully proven: `{text}`"),
                    "payload recorded as a literal with no unproven positions",
                    library
                )
            ));
        }
    }
    found
}

/// Reports an entry point that writes through the print family while
/// returning nothing, so a failed write becomes a panic rather than an
/// exit code.
pub(super) fn lost_write_error_insights(sources: &[Source]) -> Vec<Insight> {
    let library = library_knowledge();
    if !library.has(WRITE_ERROR.property) {
        return Vec::new();
    }
    let facts = behavior(sources);
    let mut found = Vec::new();
    for entry in entry_points(sources, &facts) {
        let Some(fact) = facts.get(entry.fact) else {
            continue;
        };
        if entry.returns {
            continue;
        }
        let Some(write) = fact
            .output_writes
            .iter()
            .find(|write| standard_stream(write.stream))
        else {
            continue;
        };
        found.push(Insight::new(
            fact.source,
            write.site,
            detail(
                &WRITE_ERROR,
                &fact.function,
                "the process entry point writes to a standard stream and declares no return type"
            ),
            trace(
                &WRITE_ERROR,
                &format!("`{}` declares no return type", fact.function),
                "the entry signature returns nothing, so no failure can leave it",
                library
            )
        ));
    }
    found
}

/// Whether a stream is one of the process's own standard streams, the
/// only ones these forms describe.
const fn standard_stream(stream: Stream) -> bool {
    matches!(stream, Stream::Stdout | Stream::Stderr)
}

/// Facts of one function, for the checks that need the whole record.
#[allow(dead_code, reason = "kept for the checks added next in this tier")]
type Facts = FunctionFacts;

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use std::path::PathBuf;

    use super::{
        literal_format_insights, lock_run_insights, loop_write_insights, lost_write_error_insights
    };
    use crate::source::Source;

    fn sources(text: &str) -> Vec<Source> {
        vec![Source::new(PathBuf::from("src/main.rs"), text.to_owned())]
    }

    #[test]
    fn a_run_of_prints_is_reported_and_a_single_print_is_not() {
        let run = lock_run_insights(&sources(
            "fn main() {\n    println!(\"one\");\n    println!(\"two\");\n}"
        ));
        assert_eq!(run.len(), 1);
        assert!(run[0].detail.contains("2 writes to a standard stream"));

        let single = lock_run_insights(&sources("fn main() {\n    println!(\"one\");\n}"));
        assert!(single.is_empty());
    }

    #[test]
    fn a_write_in_a_loop_is_reported_and_a_straight_one_is_not() {
        let looped = loop_write_insights(&sources(
            "fn main() {\n    for index in 0..10 { println!(\"{index}\"); }\n}"
        ));
        assert_eq!(looped.len(), 1);
        assert!(looped[0].detail.contains("buffered writer"));

        let straight = loop_write_insights(&sources("fn main() {\n    println!(\"once\");\n}"));
        assert!(straight.is_empty());
    }

    /// Both loop forms that carry a zero-times guard are reported. A bare
    /// `loop` records repetition without a guard frame, so it does not reach
    /// this insight; that boundary is stated here so a change to it is visible.
    #[test]
    fn a_guarded_loop_carrying_a_write_is_reported_in_either_form() {
        let whiled = loop_write_insights(&sources(
            "pub fn tick(mut n: u32) {\n    while n > 0 { println!(\"tick\"); n -= 1; }\n}"
        ));
        assert_eq!(whiled.len(), 1);

        let counted = loop_write_insights(&sources(
            "pub fn tick() {\n    for _ in 0..3 { println!(\"tick\"); }\n}"
        ));
        assert_eq!(counted.len(), 1);

        let bare = loop_write_insights(&sources(
            "pub fn tick() {\n    loop { println!(\"tick\"); }\n}"
        ));
        assert!(bare.is_empty());
    }

    #[test]
    fn a_constant_through_the_format_machinery_is_reported() {
        let constant = literal_format_insights(&sources("fn main() {\n    println!(\"hi\");\n}"));
        assert_eq!(constant.len(), 1);

        let computed =
            literal_format_insights(&sources("fn main() {\n    println!(\"{name}\");\n}"));
        assert!(computed.is_empty());
    }

    #[test]
    fn an_entry_point_losing_the_write_error_is_reported() {
        let lost = lost_write_error_insights(&sources("fn main() {\n    println!(\"hi\");\n}"));
        assert_eq!(lost.len(), 1);
        assert!(lost[0].detail.contains("declares no return type"));

        let returned = lost_write_error_insights(&sources(
            "fn main() -> std::io::Result<()> {\n    println!(\"hi\");\n    Ok(())\n}"
        ));
        assert!(returned.is_empty());
    }
}
