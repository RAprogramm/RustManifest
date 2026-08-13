// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! One observed write, as a single record: the stream it reaches, the
//! text it puts there, the terminator it appends, the machinery it goes
//! through, where its lock was taken, and the two readings that say how
//! certainly and how often it happens.
//!
//! One record per write site, in body order, never deduplicated — the
//! channel the effect list cannot be.

use super::{
    acquisition::Acquisition, conduit::Conduit, guard::SiteGuard, multiplicity::Multiplicity,
    payload::Payload, stream::Stream
};

/// Whether the write appends a line terminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Newline {
    /// A terminator is appended by the writing construct.
    Appended,
    /// No terminator is appended.
    None
}

/// How the destination buffers what a site writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Buffering {
    /// The stream's own declared buffering applies.
    StreamOwn,
    /// A block buffer stands in front of the stream: the destination
    /// was bound through a written buffering wrapper, so bytes reach
    /// the stream on flush, not per write.
    Block,
    /// Nothing is proven about the buffering.
    Unknown
}

/// One observed write to an output stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputWrite {
    /// Where the bytes go.
    pub stream:       Stream,
    /// How the destination buffers what this site writes.
    pub buffering:    Buffering,
    /// What is written.
    pub payload:      Payload,
    /// Whether a line terminator is appended.
    pub newline:      Newline,
    /// Byte range of the writing construct.
    pub site:         rustmanifest_schema::Range,
    /// What stands between the site and unconditional execution.
    pub guard:        SiteGuard,
    /// How many times it runs per call.
    pub multiplicity: Multiplicity,
    /// By what machinery the payload reaches the stream.
    pub conduit:      Conduit,
    /// Where the stream's lock was taken relative to this site.
    pub acquisition:  Acquisition
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

    use crate::{
        semantic::behavior::{
            FunctionFacts, Provenance, behavior,
            output::{Grade, Guard, OpaqueReason, Payload, Segment, Stream}
        },
        source::Source
    };

    /// Behavioral facts of one written snippet.
    fn facts(text: &str) -> Vec<FunctionFacts> {
        {
            let sources = [Source::new(PathBuf::from("src/lib.rs"), text.to_owned())];
            let settled = crate::semantic::settling::standard_macros_over(&sources);
            crate::semantic::behavior_with(&sources, Some(&settled))
        }
    }

    /// A hole is a hole whichever macro wrote it. Reading holes on the
    /// stream macros alone left the same binding readable through
    /// `println!` and unreadable through `writeln!` to a handle.
    #[test]
    fn a_hole_written_to_a_handle_reads_like_one_written_to_the_stream() {
        let through_stream = facts("fn go() { let value = 7_u32; println!(\"{value}\"); }");
        let through_handle = facts(
            "use std::io::Write;\nfn go(handle: &mut dyn Write) { let value = 7_u32; let _ = \
             writeln!(handle, \"{value}\"); }"
        );
        let hole_origin = |facts: &[FunctionFacts]| -> Option<Provenance> {
            facts
                .iter()
                .find(|fact| fact.function == "go")
                .and_then(|fact| fact.output_writes.first())
                .and_then(|write| match &write.payload {
                    Payload::Template(segments) => {
                        segments.iter().find_map(|segment| match segment {
                            Segment::Hole(hole) => Some(hole.origin.clone()),
                            Segment::Text(_) => None
                        })
                    }
                    _ => None
                })
        };
        let stream_origin = hole_origin(&through_stream).expect("the stream write holds a hole");
        let handle_origin = hole_origin(&through_handle).expect("the handle write holds a hole");
        assert!(stream_origin.derived());
        assert_eq!(stream_origin, handle_origin);
    }

    #[test]
    fn a_greeting_records_one_stdout_write_with_its_proven_text() {
        use std::path::PathBuf;

        use crate::{semantic::behavior::behavior, source::Source};
        let sources = [Source::new(
            PathBuf::from("src/main.rs"),
            "fn main() {\n    println!(\"Hello, world!\");\n}".to_owned()
        )];
        let facts = crate::semantic::behavior_with(&sources, Some(&crate::semantic::settling::standard_macros_over(&sources)));
        let main = facts
            .iter()
            .find(|fact| fact.function == "main")
            .expect("the entry function is a fact");
        assert_eq!(main.output_writes.len(), 1);
        let write = &main.output_writes[0];
        assert_eq!(write.stream, super::Stream::Stdout);
        assert_eq!(write.payload.fully_known(), Some("Hello, world!"));
        assert_eq!(write.newline, super::Newline::Appended);
        assert_eq!(write.guard.local_grade(), Grade::Certain);
        assert!(write.site.end > write.site.start);
        assert_eq!(main.effects, vec![crate::semantic::Effect::ConsoleIo]);
        assert_eq!(write.conduit, super::Conduit::Formatting);
        assert_eq!(write.acquisition, super::Acquisition::AtSite);
    }

    /// A handle put in a binding still names its stream, however many
    /// bindings stand between the constructor and the write. Losing the
    /// stream there would blind the engine to the very form it prescribes
    /// — lock once, write bytes — and the lock reading says the guard was
    /// taken before the site, not at it.
    #[test]
    fn a_write_through_a_bound_handle_keeps_its_stream() {
        use std::path::PathBuf;

        use crate::{semantic::behavior::behavior, source::Source};
        for body in [
            "    let mut out = std::io::stdout().lock();\n    out.write_all(b\"hi\")",
            "    let out = std::io::stdout();\n    let mut held = out.lock();\n    \
             held.write_all(b\"hi\")"
        ] {
            let sources = [Source::new(
                PathBuf::from("src/main.rs"),
                format!("fn main() -> std::io::Result<()> {{\n{body}\n}}")
            )];
            let facts = crate::semantic::behavior_with(&sources, Some(&crate::semantic::settling::standard_macros_over(&sources)));
            let main = facts
                .iter()
                .find(|fact| fact.function == "main")
                .expect("the entry function is a fact");
            assert_eq!(main.output_writes.len(), 1);
            let write = &main.output_writes[0];
            assert_eq!(write.stream, super::Stream::Stdout);
            assert_eq!(write.conduit, super::Conduit::Bytes);
            assert_eq!(write.acquisition, super::Acquisition::Held);
        }
    }

    /// A write on the handle itself takes the lock per call, so it is not
    /// read as a guard held across a run.
    #[test]
    fn a_write_on_the_handle_itself_takes_the_lock_at_the_site() {
        use std::path::PathBuf;

        use crate::{semantic::behavior::behavior, source::Source};
        let sources = [Source::new(
            PathBuf::from("src/main.rs"),
            "fn main() -> std::io::Result<()> {\n    let mut out = std::io::stdout();\n    \
             out.write_all(b\"hi\")\n}"
                .to_owned()
        )];
        let facts = crate::semantic::behavior_with(&sources, Some(&crate::semantic::settling::standard_macros_over(&sources)));
        let main = facts
            .iter()
            .find(|fact| fact.function == "main")
            .expect("the entry function is a fact");
        assert_eq!(main.output_writes.len(), 1);
        assert_eq!(main.output_writes[0].stream, super::Stream::Stdout);
        assert_eq!(
            main.output_writes[0].acquisition,
            super::Acquisition::AtSite
        );
    }

    #[test]
    fn each_write_is_its_own_event_in_body_order() {
        use std::path::PathBuf;

        use crate::{semantic::behavior::behavior, source::Source};
        let sources = [Source::new(
            PathBuf::from("src/main.rs"),
            "fn main() {\n    println!(\"one\");\n    eprintln!(\"two\");\n    println!(\"one\");\n}"
                .to_owned()
        )];
        let facts = crate::semantic::behavior_with(&sources, Some(&crate::semantic::settling::standard_macros_over(&sources)));
        let main = facts
            .iter()
            .find(|fact| fact.function == "main")
            .expect("the entry function is a fact");
        let observed: Vec<(super::Stream, Option<&str>)> = main
            .output_writes
            .iter()
            .map(|write| (write.stream, write.payload.fully_known()))
            .collect();
        assert_eq!(
            observed,
            [
                (super::Stream::Stdout, Some("one")),
                (super::Stream::Stderr, Some("two")),
                (super::Stream::Stdout, Some("one"))
            ]
        );
    }

    fn writes_of_source(source: &str, function: &str) -> Vec<super::OutputWrite> {
        use std::path::PathBuf;

        use crate::{semantic::behavior::behavior, source::Source};
        let sources = [Source::new(PathBuf::from("src/main.rs"), source.to_owned())];
        crate::semantic::behavior_with(&sources, Some(&crate::semantic::settling::standard_macros_over(&sources)))
            .into_iter()
            .find(|fact| fact.function == function)
            .map(|fact| fact.output_writes)
            .unwrap_or_default()
    }

    fn writes_of(body: &str) -> Vec<super::OutputWrite> {
        writes_of_source(&format!("fn main() {{\n{body}\n}}"), "main")
    }

    #[test]
    fn a_write_under_a_branch_is_possible_not_certain() {
        let branch = writes_of("    if flag { println!(\"hi\"); }");
        assert_eq!(branch[0].guard.guards, [Guard::If]);
        assert_eq!(branch[0].guard.local_grade(), Grade::Possible);

        let straight = writes_of("    println!(\"hi\");");
        assert!(straight[0].guard.guards.is_empty());
        assert_eq!(straight[0].guard.local_grade(), Grade::Certain);
    }

    #[test]
    fn a_loop_body_repeats_and_only_a_loop_that_may_not_run_weakens_it() {
        let counted = writes_of("    for _ in 0..n { println!(\"hi\"); }");
        assert_eq!(counted[0].guard.guards, [Guard::For]);
        assert_eq!(counted[0].multiplicity, super::Multiplicity::Unbounded);
        assert_eq!(counted[0].guard.local_grade(), Grade::Possible);

        let forever = writes_of("    loop { println!(\"hi\"); }");
        assert!(forever[0].guard.guards.is_empty());
        assert_eq!(forever[0].multiplicity, super::Multiplicity::Unbounded);
        assert_eq!(forever[0].guard.local_grade(), Grade::Certain);
    }

    #[test]
    fn a_closure_or_nested_item_body_only_runs_where_it_is_called() {
        let closure = writes_of("    let shout = || println!(\"hi\");");
        assert_eq!(closure[0].guard.guards, [Guard::Closure]);

        let nested = writes_of("    fn inner() { println!(\"hi\"); }");
        assert_eq!(nested[0].guard.guards, [Guard::NestedItem]);
    }

    #[test]
    fn a_configuration_gate_is_never_resolved_only_recorded() {
        let gated = writes_of("    #[cfg(feature = \"loud\")]\n    println!(\"hi\");");
        assert_eq!(gated[0].guard.guards, [Guard::CfgGated]);
        assert_eq!(gated[0].guard.local_grade(), Grade::Possible);
    }

    #[test]
    fn the_guard_stack_unwinds_so_a_later_write_is_not_poisoned() {
        let writes = writes_of(
            "    if a { match b { Some(_) => println!(\"deep\"), None => {} } }\n    \
             println!(\"flat\");"
        );
        assert_eq!(writes.len(), 2);
        assert_eq!(writes[0].guard.guards.len(), 2);
        assert!(writes[1].guard.guards.is_empty());
    }

    #[test]
    fn a_configuration_gate_is_recorded_on_every_statement_shape() {
        let block = writes_of("    #[cfg(feature = \"loud\")]\n    unsafe { println!(\"hi\"); }");
        assert_eq!(block[0].guard.guards, [Guard::CfgGated]);

        let repeated =
            writes_of("    #[cfg(feature = \"loud\")]\n    for _ in 0..n { println!(\"hi\"); }");
        assert_eq!(repeated[0].guard.guards, [Guard::CfgGated, Guard::For]);

        let item = writes_of("    #[cfg(test)]\n    fn inner() { println!(\"hi\"); }");
        assert_eq!(item[0].guard.guards, [Guard::CfgGated, Guard::NestedItem]);
    }

    #[test]
    fn an_exit_inside_a_nested_body_does_not_weaken_the_body_around_it() {
        let nested = writes_of("    fn inner() { return; }\n    println!(\"outer\");");
        assert_eq!(nested.len(), 1);
        assert_eq!(nested[0].guard.prior_exits, 0);
        assert_eq!(nested[0].guard.local_grade(), Grade::Certain);

        let closure = writes_of("    let quit = || return;\n    println!(\"outer\");");
        assert_eq!(closure[0].guard.prior_exits, 0);
        assert_eq!(closure[0].guard.local_grade(), Grade::Certain);
    }

    #[test]
    fn a_bare_loop_repeats_without_weakening_the_write_inside_it() {
        let forever = writes_of("    loop { println!(\"hi\"); }");
        assert!(forever[0].guard.guards.is_empty());
        assert_eq!(forever[0].guard.local_grade(), Grade::Certain);
        assert_eq!(forever[0].multiplicity, super::Multiplicity::Unbounded);
    }

    #[test]
    fn a_loop_that_nothing_breaks_out_of_kills_what_follows_it() {
        let endless = writes_of("    loop { helper(); }\n    println!(\"hi\");");
        assert_eq!(endless[0].guard.local_grade(), Grade::Dead);

        let escaping = writes_of("    loop { if flag { break; } }\n    println!(\"hi\");");
        assert!(!escaping[0].guard.dead);
        assert_eq!(escaping[0].guard.local_grade(), Grade::Possible);
    }

    #[test]
    fn a_binding_arm_is_irrefutable_only_without_a_subpattern() {
        let subpattern =
            writes_of("    match v { y @ Some(1) => println!(\"a\"), _ => println!(\"b\") }");
        assert_eq!(
            subpattern[0].guard.guards,
            [Guard::MatchArm {
                irrefutable: false,
                guarded:     false
            }]
        );
        assert_eq!(
            subpattern[1].guard.guards,
            [Guard::MatchArm {
                irrefutable: true,
                guarded:     false
            }]
        );

        let conditional =
            writes_of("    match v { y if y > 1 => println!(\"a\"), _ => println!(\"b\") }");
        assert_eq!(
            conditional[0].guard.guards,
            [Guard::MatchArm {
                irrefutable: true,
                guarded:     true
            }]
        );

        let binding = writes_of("    match v { y => println!(\"a\") }");
        assert!(binding[0].guard.guards.is_empty());
        assert_eq!(binding[0].guard.local_grade(), Grade::Certain);
    }

    #[test]
    fn an_async_body_is_framed_like_a_closure_because_both_run_where_driven() {
        let writes =
            writes_of("    let f = async { println!(\"a\"); };\n    let g = || println!(\"b\");");
        assert_eq!(writes.len(), 2);
        assert_eq!(writes[0].guard.guards, [Guard::Closure]);
        assert_eq!(writes[0].guard.local_grade(), Grade::Possible);
        assert_eq!(writes[1].guard.guards, [Guard::Closure]);
    }

    #[test]
    fn a_process_exit_reached_through_an_import_kills_what_follows_it() {
        let imported = writes_of_source(
            "use std::process::exit;\nfn main() { exit(1); println!(\"hi\"); }",
            "main"
        );
        assert!(imported[0].guard.dead);
        assert_eq!(imported[0].guard.local_grade(), Grade::Dead);

        let aborted = writes_of_source(
            "use std::process::abort;\nfn main() { abort(); println!(\"hi\"); }",
            "main"
        );
        assert!(aborted[0].guard.dead);

        let written = writes_of("    std::process::exit(1);\n    println!(\"hi\");");
        assert!(written[0].guard.dead);

        let unrelated = writes_of_source(
            "use crate::shell::exit;\nfn main() { exit(1); println!(\"hi\"); }",
            "main"
        );
        assert!(!unrelated[0].guard.dead);
    }

    #[test]
    fn a_closure_invoked_where_it_is_written_runs_its_body_there() {
        let diverging = writes_of("    (|| std::process::exit(1))();\n    println!(\"hi\");");
        assert!(diverging[0].guard.dead);

        let returning = writes_of("    (|| { return; })();\n    println!(\"hi\");");
        assert!(!returning[0].guard.dead);
        assert_eq!(returning[0].guard.prior_exits, 1);

        let held = writes_of("    let quit = || std::process::exit(1);\n    println!(\"hi\");");
        assert_eq!(held[0].guard.local_grade(), Grade::Certain);
    }

    #[test]
    fn a_nested_item_after_a_divergence_still_runs_where_it_is_called() {
        let after_exit = writes_of(
            "    helper();\n    std::process::exit(0);\n    fn helper() { println!(\"hi\"); }"
        );
        assert!(!after_exit[0].guard.dead);
        assert_eq!(after_exit[0].guard.guards, [Guard::NestedItem]);
        assert_eq!(after_exit[0].guard.local_grade(), Grade::Possible);

        let after_return =
            writes_of("    helper();\n    return;\n    fn helper() { println!(\"hi\"); }");
        assert!(!after_return[0].guard.dead);
        assert_eq!(after_return[0].guard.prior_exits, 0);
    }

    #[test]
    fn how_often_a_body_writes_comes_from_its_completion_not_from_the_keyword() {
        let leaves = writes_of("    loop { println!(\"x\"); break; }\n    println!(\"after\");");
        assert_eq!(leaves[0].payload.fully_known(), Some("x"));
        assert_eq!(leaves[0].multiplicity, super::Multiplicity::Once);

        let restarts = writes_of("    loop { println!(\"x\"); continue; }");
        assert_eq!(restarts[0].multiplicity, super::Multiplicity::Unbounded);

        let hidden = writes_of("    loop { println!(\"x\"); escape!(); }");
        assert_eq!(hidden[0].multiplicity, super::Multiplicity::Unbounded);

        let endless = writes_of("    loop { println!(\"x\"); }");
        assert_eq!(endless[0].multiplicity, super::Multiplicity::Unbounded);
    }

    #[test]
    fn a_while_condition_repeats_with_the_loop_it_governs() {
        let writes = writes_of("    while ready(println!(\"tick\")) { println!(\"body\"); }");
        assert_eq!(writes[0].payload.fully_known(), Some("tick"));
        assert!(writes[0].guard.guards.is_empty());
        assert_eq!(writes[0].multiplicity, super::Multiplicity::Unbounded);
        assert_eq!(writes[1].guard.guards, [Guard::While]);
        assert_eq!(writes[1].multiplicity, super::Multiplicity::Unbounded);
    }

    #[test]
    fn a_write_inside_a_macro_argument_is_recorded_not_dropped() {
        let nested = writes_of("    println!(\"{}\", { println!(\"inner\"); 1 });");
        assert_eq!(nested.len(), 2);
        assert_eq!(nested[1].payload.fully_known(), Some("inner"));

        let asserted = writes_of("    assert!(flag, \"{}\", { println!(\"inner\"); 1 });");
        assert_eq!(asserted.len(), 1);
        assert_eq!(asserted[0].payload.fully_known(), Some("inner"));

        let collected = writes_of("    let v = vec![{ println!(\"inner\"); 1 }];");
        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0].payload.fully_known(), Some("inner"));
    }

    /// Whether a call comes back to its caller is the compiler's answer,
    /// stated as the type of the call itself: a call the resolver renders
    /// as the never type is reached and never left, so nothing written
    /// after it runs.
    #[test]
    fn a_call_to_a_never_returning_function_kills_what_follows_it() {
        use std::path::PathBuf;

        use crate::{
            semantic::{behavior::behavior_with, settling::Settling},
            source::Source
        };
        let dying = "fn dies() -> ! { panic!(\"stop\") }\nfn main() {\n    dies();\n                 println!(\"hi\");\n}";
        let sources = [Source::new(PathBuf::from("src/main.rs"), dying.to_owned())];
        let leaving = Settling::over("src/main.rs", dying)
            .after("fn main")
            .typed("dies()", "!")
            .done();
        let mut leaving = leaving;
        crate::semantic::settling::settle_declared(&mut leaving, &sources);
        crate::semantic::settling::settle_standard_macros(&mut leaving, &sources);
        let facts = behavior_with(&sources, Some(&leaving));
        let main = facts
            .iter()
            .find(|fact| fact.function == "main")
            .expect("the entry function is a fact");
        assert_eq!(main.output_writes.len(), 1);
        assert_eq!(main.output_writes[0].guard.local_grade(), Grade::Dead);

        let coming_back = "fn helper() -> u8 { 1 }\nfn main() {\n    helper();\n    println!(\"hi\");\n}";
        let returning = [Source::new(
            PathBuf::from("src/main.rs"),
            coming_back.to_owned()
        )];
        let settled = Settling::over("src/main.rs", coming_back)
            .after("fn main")
            .typed("helper()", "u8")
            .done();
        let mut settled = settled;
        crate::semantic::settling::settle_declared(&mut settled, &returning);
        crate::semantic::settling::settle_standard_macros(&mut settled, &returning);
        let plain = behavior_with(&returning, Some(&settled));
        let entry = plain
            .iter()
            .find(|fact| fact.function == "main")
            .expect("the entry function is a fact");
        assert_eq!(entry.output_writes[0].guard.local_grade(), Grade::Certain);
    }

    #[test]
    fn a_writer_macro_with_no_arguments_beyond_its_destination_is_read_without_panicking() {
        let writes = writes_of("    let mut out = String::new();\n    write!(out, \"hi\").ok();");
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0].payload.fully_known(), Some("hi"));
    }
}
