// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a program takes in: the values it draws from outside the process,
//! one record per site.
//!
//! The mirror of the write channel, and built on the same terms: one
//! record per reading construct, in body order, never deduplicated, each
//! carrying what stands over it and nothing beyond what the source
//! proves. The effect list can say the environment was touched and can
//! say no more — not which channel, not which part of it, not whether the
//! read happens at all.
//!
//! The honesty rules are the write channel's own. A channel that cannot
//! be identified is recorded as such rather than omitted; a part drawn
//! through a construct that does not fix which part says so while the
//! channel stays proven; and the version of the recognized source list is
//! stamped into every claim, so absence reads as "the recognizer found
//! none" and never as "the program reads nothing".

mod channel;
mod intake;
mod read;
mod selector;

pub use self::{
    channel::Channel,
    intake::{Intake, ReadShape, Terminator},
    read::InputRead,
    selector::{Selector, SelectorGap}
};

/// Version of the recognized read-source list. Stamped into claims so a
/// negative result states which recognizer produced it.
pub const INPUT_SOURCE_SET: u32 = 1;

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use std::path::PathBuf;

    use super::{Channel, InputRead, Intake, ReadShape, Selector, SelectorGap, Terminator};
    use crate::{
        semantic::behavior::Acquisition,
        source::Source
    };

    fn reads(body: &str) -> Vec<InputRead> {
        let text = format!("use std::env;\nuse std::io::BufRead;\nfn main() {{\n{body}\n}}");
        let sources = [Source::new(PathBuf::from("src/main.rs"), text)];
        crate::semantic::behavior_with(&sources, Some(&crate::semantic::settling::standard_macros_over(&sources)))
            .iter()
            .find(|fact| fact.function == "main")
            .map(|fact| fact.input_reads.clone())
            .expect("the entry function is a fact")
    }

    /// The words a process was started with are a read of the command
    /// line, and the library's validation of them as text is part of the
    /// record.
    #[test]
    fn the_words_a_process_was_started_with_are_a_read() {
        let recorded = reads("    let _ = env::args();");
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].channel, Channel::CommandLine);
        assert_eq!(recorded[0].selector, Selector::Whole);
        assert_eq!(recorded[0].shape, ReadShape::Sequence);
        assert_eq!(recorded[0].intake, Intake::Text);
    }

    /// The two ways of taking the same words differ in one field only,
    /// and the difference is a real one: one is validated as text on the
    /// way in and the other is not.
    #[test]
    fn the_raw_and_validated_intakes_differ_only_in_the_intake() {
        let validated = reads("    let _ = env::args();");
        let raw = reads("    let _ = env::args_os();");
        assert_eq!(validated[0].intake, Intake::Text);
        assert_eq!(raw[0].intake, Intake::Raw);
        assert_eq!(validated[0].channel, raw[0].channel);
        assert_eq!(validated[0].shape, raw[0].shape);
    }

    /// An entry of the environment is named where the source writes its
    /// key, and refuses where it does not.
    #[test]
    fn a_keyed_lookup_names_its_entry_only_where_the_key_is_written() {
        assert_eq!(
            reads("    let _ = env::var(\"HOME\");")[0].selector,
            Selector::Named("HOME".to_owned())
        );
        assert_eq!(
            reads("    let _ = env::var(chosen());")[0].selector,
            Selector::NotIdentified(SelectorGap::RuntimeValue)
        );
    }

    /// A construct written over a read is folded into it: the boundary
    /// survives, and one read stays one record.
    #[test]
    fn a_construct_written_over_a_read_is_folded_into_it() {
        let collected = reads("    let _: Vec<String> = env::args().collect();");
        assert_eq!(collected.len(), 1, "one read, not one per construct");
        assert_eq!(collected[0].channel, Channel::CommandLine);
        assert_eq!(collected[0].selector, Selector::Whole);
    }

    /// An element singled out by a written position is that position of
    /// the channel, which is what a program taking arguments does.
    #[test]
    fn an_element_at_a_written_position_names_that_position() {
        let picked = reads("    let _ = env::args().nth(1);");
        assert_eq!(picked.len(), 1);
        assert_eq!(picked[0].selector, Selector::Positional(1));
        assert_eq!(picked[0].shape, ReadShape::Element);
    }

    /// A construct that does not keep positions leaves the channel proven
    /// and the part drawn unproven — degradation field by field, never a
    /// dropped record.
    #[test]
    fn a_construct_that_loses_positions_keeps_the_channel_and_drops_the_part() {
        let filtered = reads("    let _ = env::args().filter(keep).nth(1);");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].channel, Channel::CommandLine);
        assert_eq!(
            filtered[0].selector,
            Selector::NotIdentified(SelectorGap::Advanced),
            "a position of the filtered sequence is not a position of the channel"
        );
    }

    /// The process's own input stream is a channel of its own, and the
    /// lock reading is the write channel's own reading.
    #[test]
    fn a_read_of_the_input_stream_names_its_channel_and_its_lock() {
        let direct = reads(
            "    let mut line = String::new();\n    let _ = \
                            std::io::stdin().read_line(&mut line);"
        );
        assert_eq!(direct[0].channel, Channel::StandardInput);
        assert_eq!(direct[0].terminator, Terminator::Retained);
        assert_eq!(direct[0].acquisition, Acquisition::AtSite);

        let held = reads(
            "    let handle = std::io::stdin();\n    let guard = handle.lock();\n    \
                          let _ = guard.lines();"
        );
        assert_eq!(held[0].channel, Channel::StandardInput);
        assert_eq!(held[0].terminator, Terminator::Stripped);
        assert_eq!(held[0].acquisition, Acquisition::Held);
    }

    /// A program's own function named like the library's is not the
    /// library's, so nothing is recorded for it.
    #[test]
    fn a_namesake_of_the_program_is_not_the_boundary() {
        let sources = [Source::new(
            PathBuf::from("src/main.rs"),
            "mod own { pub fn args() -> Vec<String> { Vec::new() } }\nfn main() { let _ = \
             own::args(); }"
                .to_owned()
        )];
        let facts = crate::semantic::behavior_with(&sources, Some(&crate::semantic::settling::standard_macros_over(&sources)));
        let entry = facts
            .iter()
            .find(|fact| fact.function == "main")
            .expect("the entry function is a fact");
        assert!(entry.input_reads.is_empty());
    }
}
