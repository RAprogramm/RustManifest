// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The write standing inside a loop, where the stream's own buffering
//! turns every iteration into a separate write syscall.

use super::{
    catalogue::{LOOP_WRITE, library_knowledge, support},
    finding::{detail, trace},
    shipped_code::shipped
};
use crate::{
    semantic::{
        behavior::{Guard, Multiplicity, behavior},
        insight::Insight
    },
    source::Source
};

/// Reports writes inside a loop, where the stream's own buffering turns
/// every iteration into a separate write syscall.
///
/// The supporting property is the buffering of the stream the write
/// actually goes to, so a write to the error stream cites the error
/// stream's declaration and never the output stream's.
pub(in crate::semantic) fn loop_write_insights(sources: &[Source]) -> Vec<Insight> {
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
                    ),
                    1
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
}
