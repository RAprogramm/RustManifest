// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The write standing inside a loop, where the stream's own buffering
//! turns every iteration into a separate write syscall.

mod buffering;

use super::{
    catalogue::{LOOP_WRITE, library_knowledge, support},
    finding::{detail, trace},
    shipped_code::shipped
};
use crate::semantic::{
    behavior::{Buffering, Guard, Multiplicity},
    insight::Insight,
    workspace::Workspace
};

/// Reports writes inside a loop, where the stream's own buffering turns
/// every iteration into a separate write syscall.
///
/// The supporting property is the buffering of the stream the write
/// actually goes to, so a write to the error stream cites the error
/// stream's declaration and never the output stream's.
pub(in crate::semantic) fn loop_write_insights(workspace: &Workspace<'_>) -> Vec<Insight> {
    let sources = workspace.sources();
    let library = library_knowledge();
    let mut found = Vec::new();
    for fact in workspace.facts() {
        if !shipped(sources, fact) {
            continue;
        }
        for write in &fact.output_writes {
            let Some(derived) = support(&LOOP_WRITE, write.stream, library) else {
                continue;
            };
            if write.multiplicity != Multiplicity::Unbounded {
                continue;
            }
            if write.buffering == Buffering::Block {
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
            found.push(
                Insight::new(
                    fact.source,
                    write.site,
                    detail(
                        &LOOP_WRITE,
                        &fact.function,
                        &format!(
                            "a write to {} stands under {loops} loop construct(s)",
                            write.stream
                        ),
                        1
                    ),
                    trace(
                        &format!("the write site stands under {loops} loop construct(s)"),
                        "recorded guard stack of the write site",
                        derived
                    )
                )
                .with_fixes(
                    sources.get(fact.source).map_or_else(Vec::new, |source| {
                        buffering::buffer_fixes(source, write.site)
                    })
                )
            );
        }
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

    use super::loop_write_insights;
    use crate::source::Source;

    fn sources(text: &str) -> Vec<Source> {
        vec![Source::new(PathBuf::from("src/main.rs"), text.to_owned())]
    }

    /// The insights of a fixture read against what a compiler settles
    /// about it: the standard macros it writes and the types it
    /// declares, without which the readings above answer nothing.
    fn insights_over(sources: &[Source]) -> Vec<crate::semantic::insight::Insight> {
        let settled = crate::semantic::settling::settled_over(sources);
        loop_write_insights(&crate::semantic::workspace::Workspace::resolved(
            sources, &settled
        ))
    }

    /// The locked-handle shape with a written flush carries the
    /// buffering fix: one edit wrapping the initializer.
    #[test]
    fn a_locked_and_flushed_handle_carries_the_buffering_fix() {
        let text = "use std::io::Write;\nfn main() {\n    let stdout = std::io::stdout();\n    \
                    let mut handle = stdout.lock();\n    for line in 0..9 {\n        \
                    writeln!(handle, \"{line}\").ok();\n    }\n    handle.flush().ok();\n}";
        let found = insights_over(&sources(text));
        let fixes = &found[0].fixes;
        assert_eq!(fixes.len(), 1, "{fixes:?}");
        assert_eq!(
            fixes[0].replacement,
            "std::io::BufWriter::new(stdout.lock())"
        );
        let replaced = &text[fixes[0].range.start as usize..fixes[0].range.end as usize];
        assert_eq!(replaced, "stdout.lock()");
    }

    /// Without a written flush the buffer would drain on drop with its
    /// errors swallowed, so no fix is synthesized.
    #[test]
    fn a_handle_without_a_flush_gets_no_fix() {
        let text = "use std::io::Write;\nfn main() {\n    let stdout = std::io::stdout();\n    \
                    let mut handle = stdout.lock();\n    for line in 0..9 {\n        \
                    writeln!(handle, \"{line}\").ok();\n    }\n}";
        let found = insights_over(&sources(text));
        assert!(found[0].fixes.is_empty(), "{:?}", found[0].fixes);
    }

    /// A handle that also leaves as a plain value is not provably a
    /// write-only handle, so no fix is synthesized.
    #[test]
    fn a_handle_used_beyond_writing_gets_no_fix() {
        let text = "use std::io::Write;\nfn keep(handle: &std::io::StdoutLock<'_>) { let _ = \
                    handle; }\nfn main() {\n    let stdout = std::io::stdout();\n    let mut \
                    handle = stdout.lock();\n    for line in 0..9 {\n        writeln!(handle, \
                    \"{line}\").ok();\n    }\n    keep(&handle);\n    handle.flush().ok();\n}";
        let found = insights_over(&sources(text));
        assert!(found[0].fixes.is_empty(), "{:?}", found[0].fixes);
    }

    /// A loop writing through a written block buffer is the prescribed
    /// form: the stream is known, the buffering is known, and the
    /// silence rests on that knowledge rather than on an unresolved
    /// destination.
    #[test]
    fn a_buffered_loop_write_is_silent_by_knowledge() {
        let found = insights_over(&sources(
            "use std::io::Write;\nfn main() {\n    let mut out = \
             std::io::BufWriter::new(std::io::stdout().lock());\n    for line in 0..9 {\n        \
             writeln!(out, \"{line}\").ok();\n    }\n    out.flush().ok();\n}"
        ));
        assert!(found.is_empty(), "{found:?}");
    }

    /// The two-step spelling — the stream held in its own binding, the
    /// buffer bound over its lock — resolves the same as the inline one.
    #[test]
    fn a_buffered_write_over_a_bound_stream_is_silent_too() {
        let found = insights_over(&sources(
            "use std::io::Write;\nfn main() {\n    let stdout = std::io::stdout();\n    let mut \
             out = std::io::BufWriter::new(stdout.lock());\n    for line in 0..9 {\n        \
             writeln!(out, \"{line}\").ok();\n    }\n    out.flush().ok();\n}"
        ));
        assert!(found.is_empty(), "{found:?}");
    }

    /// A bare println in a loop has no handle to buffer, so the finding
    /// stays a suggestion.
    #[test]
    fn a_bare_println_in_a_loop_gets_no_fix() {
        let found = insights_over(&sources(
            "fn main() {\n    for line in 0..9 {\n        println!(\"{line}\");\n    }\n}"
        ));
        assert!(found[0].fixes.is_empty(), "{:?}", found[0].fixes);
    }

    #[test]
    fn a_write_in_a_loop_is_reported_and_a_straight_one_is_not() {
        let looped = insights_over(&sources(
            "fn main() {\n    for index in 0..10 { println!(\"{index}\"); }\n}"
        ));
        assert_eq!(looped.len(), 1);
        assert!(looped[0].detail.contains("buffered writer"));

        let straight = insights_over(&sources("fn main() {\n    println!(\"once\");\n}"));
        assert!(straight.is_empty());
    }

    /// Both loop forms that carry a zero-times guard are reported. A bare
    /// `loop` records repetition without a guard frame, so it does not reach
    /// this insight; that boundary is stated here so a change to it is visible.
    #[test]
    fn a_guarded_loop_carrying_a_write_is_reported_in_either_form() {
        let whiled = insights_over(&sources(
            "pub fn tick(mut n: u32) {\n    while n > 0 { println!(\"tick\"); n -= 1; }\n}"
        ));
        assert_eq!(whiled.len(), 1);

        let counted = insights_over(&sources(
            "pub fn tick() {\n    for _ in 0..3 { println!(\"tick\"); }\n}"
        ));
        assert_eq!(counted.len(), 1);

        let bare = insights_over(&sources(
            "pub fn tick() {\n    loop { println!(\"tick\"); }\n}"
        ));
        assert!(bare.is_empty());
    }

    /// A loop write on the error stream must cite the error stream's own
    /// declaration — that stream is unbuffered, and the output stream's
    /// line buffering proves nothing about it.
    #[test]
    fn a_loop_write_cites_the_buffering_of_its_own_stream() {
        let out = insights_over(&sources(
            "pub fn dump(lines: &[String]) {\n    for line in lines { println!(\"{line}\"); }\n}"
        ));
        assert_eq!(out.len(), 1);
        assert!(out[0].detail.contains("a write to stdout"));
        assert!(out[0].trace[1].fact.contains("LineWriter"));
        assert!(out[0].trace[1].reason.contains("stdio.rs"));

        let err = insights_over(&sources(
            "pub fn warn(lines: &[String]) {\n    for line in lines { eprintln!(\"{line}\"); }\n}"
        ));
        assert_eq!(err.len(), 1);
        assert!(err[0].detail.contains("a write to stderr"));
        assert!(err[0].trace[1].fact.contains("StderrRaw"));
        assert!(!err[0].trace[1].fact.contains("LineWriter"));
    }
}
