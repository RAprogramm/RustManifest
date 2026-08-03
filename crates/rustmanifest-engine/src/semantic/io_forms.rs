// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Canonical forms of writing to a standard stream, and the cost of
//! deviating from them.
//!
//! The forms live in this module as data: the properties the code must
//! satisfy, the shape that satisfies them, what one deviation costs, and
//! the library guarantee the cost follows from. A check contributes
//! only facts — which function, which site, how many, what text — and
//! the message is assembled mechanically by [`detail`] from those facts
//! and the catalogue entry. No check writes a sentence of its own, so a
//! finding can never claim more than the catalogue and the fact base
//! together license.
//!
//! Every form names its supporting property *per stream*. The two
//! standard streams are two objects in the library — two handles, two
//! writer chains, two independent locks — so a property proven of one is
//! no evidence about the other, and a form whose property was not
//! derived for the stream a write goes to produces no finding at that
//! site.
//!
//! What the catalogue cannot derive, and does not pretend to: the
//! `canonical` shape and the `cost` sentence are this repository's
//! prescription and its rationale, written here in prose. Nothing in the
//! library's sources states either — a library says what it does, never
//! what a caller ought to do instead — so no derivation is attempted for
//! them. What the library does state is the property and its location,
//! and that is what a finding quotes: the trace step carries the
//! library's own text at the recorded range, not a restatement of it.

use rustmanifest_schema::TraceStep;

use super::{
    behavior::{
        FunctionFacts, Grade, Guard, Multiplicity, OutputWrite, Payload, Stream, behavior
    },
    entry::entry_points,
    insight::Insight,
    std_derive::{StdFact, StdOutputKnowledge}
};
use crate::source::Source;

/// One canonical form and the cost of deviating from it.
///
/// The library guarantee behind a form is never written here: it is
/// derived from the standard library's own sources and cited by property
/// name, so a form whose property the derivation cannot find for a
/// stream produces no finding about that stream at all.
struct Form {
    /// Derived properties that license this form for one stream, in the
    /// order they are consulted. An empty list means the form says
    /// nothing about that stream.
    properties: fn(Stream) -> &'static [&'static str],
    /// The shape that satisfies the property. Editorial: prescription,
    /// not derivation.
    canonical:  &'static str,
    /// What one deviation costs at run time. Editorial: the reading of
    /// the derived property, not the property itself.
    cost:       &'static str
}

/// Catalogue of output forms, indexed by the check that reads it.
const LOCK_RUN: Form = Form {
    properties: lock_properties,
    canonical:  "lock the stream handle once, then write every line through that guard",
    cost:       "one reentrant lock acquisition and release per write"
};

/// Form for writes standing inside a loop.
const LOOP_WRITE: Form = Form {
    properties: buffering_properties,
    canonical:  "wrap the locked handle in a buffered writer and flush once after the loop",
    cost:       "one write syscall per iteration"
};

/// Form for constants carried through the formatting machinery.
const CONSTANT_TEXT: Form = Form {
    properties: format_properties,
    canonical:  "write the constant's bytes directly to the locked handle",
    cost:       "one format-argument construction and piece walk per write"
};

/// Form for an entry point that cannot carry a write failure out.
const WRITE_ERROR: Form = Form {
    properties: panic_properties,
    canonical:  "return a result from the entry point and hand the write's result back",
    cost:       "a panic and stack unwind instead of an exit code"
};

/// Each stream holds its own lock, so each proves its own locking
/// separately.
const fn lock_properties(stream: Stream) -> &'static [&'static str] {
    match stream {
        Stream::Stdout => &["lock-per-call:stdout"],
        Stream::Stderr => &["lock-per-call:stderr"],
        Stream::CallerSupplied | Stream::Unknown => &[]
    }
}

/// A write reaches the operating system on every call when the stream
/// buffers by line — every print ends a line — or does not buffer at
/// all. A stream buffering by block amortizes the writes, so
/// `block-buffered:*` is deliberately absent from these lists and the
/// form stays silent where the derivation reports it.
const fn buffering_properties(stream: Stream) -> &'static [&'static str] {
    match stream {
        Stream::Stdout => &["line-buffered:stdout", "unbuffered:stdout"],
        Stream::Stderr => &["line-buffered:stderr", "unbuffered:stderr"],
        Stream::CallerSupplied | Stream::Unknown => &[]
    }
}

/// Both standard streams reach the operating system through one generic
/// print function, so the properties read out of that function hold for
/// either stream.
const fn format_properties(stream: Stream) -> &'static [&'static str] {
    match stream {
        Stream::Stdout | Stream::Stderr => &["format-machinery"],
        Stream::CallerSupplied | Stream::Unknown => &[]
    }
}

/// Same generic print function, same reasoning as
/// [`format_properties`].
const fn panic_properties(stream: Stream) -> &'static [&'static str] {
    match stream {
        Stream::Stdout | Stream::Stderr => &["panic-on-failure"],
        Stream::CallerSupplied | Stream::Unknown => &[]
    }
}

/// The derived fact licensing one form for one stream, or `None` when
/// the library sources proved nothing that licenses it there.
fn support<'a>(
    form: &Form,
    stream: Stream,
    library: &'a StdOutputKnowledge
) -> Option<&'a StdFact> {
    (form.properties)(stream)
        .iter()
        .find_map(|property| library.fact(property))
}

/// Assembles the message of one finding from facts and a catalogue
/// entry, in a fixed shape no check may deviate from.
fn detail(form: &Form, subject: &str, observed: &str) -> String {
    format!(
        "`{subject}` — {observed}\n    required: {}\n    cost: {}",
        form.canonical, form.cost
    )
}

/// Assembles the trace: the observed fact, the library's own source text
/// at the location the property was derived from, and the library's own
/// sentence about that property when it writes one.
fn trace(observed: &str, evidence: &str, derived: &StdFact) -> Vec<TraceStep> {
    let mut steps = vec![
        TraceStep {
            fact:   observed.to_owned(),
            reason: evidence.to_owned()
        },
        TraceStep {
            fact:   format!("`{}`", derived.snippet),
            reason: format!(
                "{}:{} — {}",
                derived.file.display(),
                derived.line,
                derived.observed
            )
        },
    ];
    if let Some(quote) = &derived.doc {
        steps.push(TraceStep {
            fact:   quote.text.clone(),
            reason: format!(
                "{}:{} — the library's own documentation of this property",
                quote.file.display(),
                quote.line
            )
        });
    }
    steps
}

/// Library knowledge for the active toolchain, derived once per process
/// and shared by every check, or nothing when the sources are not
/// installed.
fn library_knowledge() -> &'static StdOutputKnowledge {
    static DERIVED: std::sync::OnceLock<StdOutputKnowledge> = std::sync::OnceLock::new();
    DERIVED.get_or_init(|| {
        super::std_derive::std_source_root()
            .map(|root| super::std_derive::output_knowledge(&root))
            .unwrap_or_default()
    })
}

/// How many prints in one function make a run worth locking once.
const RUN_THRESHOLD: usize = 2;

/// The streams a form can speak about, in report order.
const STANDARD_STREAMS: [Stream; 2] = [Stream::Stdout, Stream::Stderr];

/// Whether the facts of one function describe shipped code, the only
/// code these forms prescribe a shape for.
///
/// Two things disqualify a body. The attribute flag catches an item
/// inside a `#[cfg(test)]` module. The path catches what the attribute
/// cannot: a file compiled into the crate's test target carries no
/// attribute at all — compile-only fixture data for a macro test is a
/// whole file of ordinary items, shipped nowhere and run never — and
/// cargo decides that by directory, so this reads the same directory.
/// The first target directory on the path decides, as everywhere else in
/// this engine; a file under none of them stays judged.
fn shipped(sources: &[Source], fact: &FunctionFacts) -> bool {
    if fact.flags.test() {
        return false;
    }
    let Some(source) = sources.get(fact.source) else {
        return false;
    };
    !source
        .path()
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .find_map(|component| match component {
            "tests" => Some(true),
            "src" | "benches" | "examples" => Some(false),
            _ => None
        })
        .unwrap_or(false)
}

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
pub(super) fn lock_run_insights(sources: &[Source]) -> Vec<Insight> {
    let library = library_knowledge();
    let mut found = Vec::new();
    for fact in behavior(sources) {
        if !shipped(sources, &fact) {
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
                .filter(|write| write.stream == stream && !counted_in_run(write))
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
            found.push(Insight::new(
                fact.source,
                first.site,
                detail(&LOCK_RUN, &fact.function, &observed),
                steps
            ));
        }
    }
    found
}

/// Whether a write site belongs to the run its body always performs: no
/// branch stands over it, and it is not proven unreachable.
const fn counted_in_run(write: &OutputWrite) -> bool {
    write.guard.guards.is_empty() && !write.guard.dead
}

/// Reports writes inside a loop, where the stream's own buffering turns
/// every iteration into a separate write syscall.
///
/// The supporting property is the buffering of the stream the write
/// actually goes to, so a write to the error stream cites the error
/// stream's declaration and never the output stream's.
pub(super) fn loop_write_insights(sources: &[Source]) -> Vec<Insight> {
    let library = library_knowledge();
    let mut found = Vec::new();
    for fact in behavior(sources) {
        if !shipped(sources, &fact) {
            continue;
        }
        for write in &fact.output_writes {
            let Some(derived) = support(&LOOP_WRITE, write.stream, library) else {
                continue;
            };
            if write.multiplicity != Multiplicity::Unbounded {
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
                    &format!("the write site stands under {loops} loop construct(s)"),
                    "recorded guard stack of the write site",
                    derived
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
    let mut found = Vec::new();
    for fact in behavior(sources) {
        if !shipped(sources, &fact) {
            continue;
        }
        for write in &fact.output_writes {
            let Some(derived) = support(&CONSTANT_TEXT, write.stream, library) else {
                continue;
            };
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
                    &format!("the written text is fully proven: `{text}`"),
                    "payload recorded as a literal with no unproven positions",
                    derived
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
    let facts = behavior(sources);
    let mut found = Vec::new();
    for entry in entry_points(sources, &facts) {
        let Some(fact) = facts.get(entry.fact) else {
            continue;
        };
        if entry.returns || !shipped(sources, fact) {
            continue;
        }
        let Some((write, derived)) = fact
            .output_writes
            .iter()
            .find_map(|write| support(&WRITE_ERROR, write.stream, library).map(|of| (write, of)))
        else {
            continue;
        };
        found.push(Insight::new(
            fact.source,
            write.site,
            detail(
                &WRITE_ERROR,
                &fact.function,
                &format!(
                    "the process entry point writes to {} and declares no return type",
                    write.stream
                )
            ),
            trace(
                &format!("`{}` declares no return type", fact.function),
                "the entry signature returns nothing, so no failure can leave it",
                derived
            )
        ));
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

    use super::{
        CONSTANT_TEXT, LOCK_RUN, LOOP_WRITE, STANDARD_STREAMS, WRITE_ERROR, library_knowledge,
        literal_format_insights, lock_run_insights, loop_write_insights,
        lost_write_error_insights, support
    };
    use crate::{semantic::behavior::Stream, source::Source};

    fn sources(text: &str) -> Vec<Source> {
        vec![Source::new(PathBuf::from("src/main.rs"), text.to_owned())]
    }

    #[test]
    fn a_run_of_prints_is_reported_and_a_single_print_is_not() {
        let run = lock_run_insights(&sources(
            "fn main() {\n    println!(\"one\");\n    println!(\"two\");\n}"
        ));
        assert_eq!(run.len(), 1);
        assert!(run[0].detail.contains("2 writes to stdout"));

        let single = lock_run_insights(&sources("fn main() {\n    println!(\"one\");\n}"));
        assert!(single.is_empty());
    }

    /// The two standard streams hold two independent locks, so one guard
    /// could not serve both and a print to each is not a run.
    #[test]
    fn the_two_standard_streams_are_never_pooled_into_one_run() {
        let mixed = lock_run_insights(&sources(
            "fn main() {\n    println!(\"out\");\n    eprintln!(\"err\");\n}"
        ));
        assert!(mixed.is_empty());

        let both = lock_run_insights(&sources(
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
        let exclusive = lock_run_insights(&sources(
            "fn main(ok: bool) {\n    if ok { println!(\"yes\"); } else { println!(\"no\"); }\n}"
        ));
        assert!(exclusive.is_empty());

        let arms = lock_run_insights(&sources(
            "fn main(v: Command) {\n    match v {\n        Command::Place(_) => \
             println!(\"placing\"),\n        Command::Cancel(_) => println!(\"cancelling\")\n    \
             }\n}"
        ));
        assert!(arms.is_empty());

        let dead = lock_run_insights(&sources(
            "fn main() {\n    return;\n    println!(\"a\");\n    println!(\"b\");\n}"
        ));
        assert!(dead.is_empty());
    }

    /// A branch-guarded site left out of the count is still stated, so the
    /// reader knows the run reported is not the whole story.
    #[test]
    fn sites_left_out_of_a_run_are_stated_in_the_trace() {
        let mixed = lock_run_insights(&sources(
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

    /// A site standing after an exit still belongs to the run: reaching it
    /// means every unguarded site before it has already written.
    #[test]
    fn a_run_after_an_early_exit_claims_only_the_paths_that_reach_it() {
        let weakened = lock_run_insights(&sources(
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

    /// A loop write on the error stream must cite the error stream's own
    /// declaration — that stream is unbuffered, and the output stream's
    /// line buffering proves nothing about it.
    #[test]
    fn a_loop_write_cites_the_buffering_of_its_own_stream() {
        let out = loop_write_insights(&sources(
            "pub fn dump(lines: &[String]) {\n    for line in lines { println!(\"{line}\"); }\n}"
        ));
        assert_eq!(out.len(), 1);
        assert!(out[0].detail.contains("a write to stdout"));
        assert!(out[0].trace[1].fact.contains("LineWriter"));
        assert!(out[0].trace[1].reason.contains("stdio.rs"));

        let err = loop_write_insights(&sources(
            "pub fn warn(lines: &[String]) {\n    for line in lines { eprintln!(\"{line}\"); }\n}"
        ));
        assert_eq!(err.len(), 1);
        assert!(err[0].detail.contains("a write to stderr"));
        assert!(err[0].trace[1].fact.contains("StderrRaw"));
        assert!(!err[0].trace[1].fact.contains("LineWriter"));
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

    /// Compile-only fixture data for a macro test carries no
    /// `#[cfg(test)]` anywhere — it is a whole file of ordinary items
    /// that only ever reaches a compiler inside a test — so the
    /// attribute flag lets it through and the target directory has to
    /// decide. The identical body under `src` stays judged.
    #[test]
    fn a_file_in_the_test_target_is_not_shipped_code() {
        let body = "fn main() {\n    println!(\"one\");\n    println!(\"two\");\n}";
        let fixture = vec![Source::new(
            PathBuf::from("crates/thing/tests/cases/pass/commands.rs"),
            body.to_owned()
        )];
        assert!(lock_run_insights(&fixture).is_empty());
        assert!(literal_format_insights(&fixture).is_empty());
        assert!(lost_write_error_insights(&fixture).is_empty());

        let looped = vec![Source::new(
            PathBuf::from("crates/thing/tests/cases/pass/loop.rs"),
            "pub fn go(v: &[u8]) {\n    for b in v { println!(\"{b}\"); }\n}".to_owned()
        )];
        assert!(loop_write_insights(&looped).is_empty());

        let shipped = vec![Source::new(
            PathBuf::from("crates/thing/src/main.rs"),
            body.to_owned()
        )];
        assert_eq!(lock_run_insights(&shipped).len(), 1);
        assert_eq!(lost_write_error_insights(&shipped).len(), 1);
    }

    /// Path of one bundled rule's fixture side.
    fn fixture(rule: &str, side: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("the engine crate has a parent directory")
            .join("rustmanifest-rules-core")
            .join("rules")
            .join(rule)
            .join(format!("{side}.rs"))
    }

    /// Sources of one fixture side, and the standard-stream writes the
    /// engine records in it.
    fn fixture_sources(rule: &str, side: &str) -> (Vec<Source>, usize) {
        let file = fixture(rule, side);
        let text = std::fs::read_to_string(&file).expect("the fixture is readable");
        let sources = vec![Source::new(file, text)];
        let recorded = crate::semantic::behavior::behavior(&sources)
            .iter()
            .map(|fact| {
                fact.output_writes
                    .iter()
                    .filter(|write| matches!(write.stream, Stream::Stdout | Stream::Stderr))
                    .count()
            })
            .sum();
        (sources, recorded)
    }

    /// A canonical fixture that holds nothing this engine records proves
    /// nothing: it would pass with the deviation in it just as well. Both
    /// sides of the two stream-shaped rules must therefore carry writes
    /// the fact base sees.
    #[test]
    fn the_output_fixtures_hold_writes_this_engine_records() {
        for side in ["pass", "fail"] {
            let (sources, recorded) = fixture_sources("RM-IO-003", side);
            assert!(recorded > 0, "RM-IO-003/{side}.rs records no write at all");
            let found = loop_write_insights(&sources);
            if side == "pass" {
                assert!(found.is_empty(), "RM-IO-003/pass.rs must stay silent");
            } else {
                assert_eq!(
                    found.len(),
                    2,
                    "both loops of RM-IO-003/fail.rs are findings"
                );
            }

            let (sources, recorded) = fixture_sources("RM-IO-002", side);
            assert!(recorded > 0, "RM-IO-002/{side}.rs records no write at all");
            let found = lock_run_insights(&sources);
            if side == "pass" {
                assert!(
                    found.is_empty(),
                    "RM-IO-002/pass.rs must stay silent but reported {found:?}"
                );
            } else {
                assert_eq!(
                    found.len(),
                    1,
                    "the one run of RM-IO-002/fail.rs is a finding"
                );
            }
        }
    }

    /// Every finding quotes the library at the place its property was
    /// derived from, and no form speaks about a stream the derivation
    /// says nothing about.
    #[test]
    fn every_form_is_backed_by_derived_library_text() {
        let library = library_knowledge();
        for form in [&LOCK_RUN, &LOOP_WRITE, &CONSTANT_TEXT, &WRITE_ERROR] {
            for stream in [Stream::CallerSupplied, Stream::Unknown] {
                assert!(support(form, stream, library).is_none());
            }
            for stream in STANDARD_STREAMS {
                let derived = support(form, stream, library)
                    .expect("every form must be licensed for both standard streams");
                assert!(!derived.snippet.is_empty());
                assert!(derived.line > 0);
            }
        }
    }
}
