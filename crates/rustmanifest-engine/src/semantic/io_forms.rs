// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Canonical forms of writing to a standard stream, and the cost of
//! deviating from them.
//!
//! The forms live in [`catalogue`] as data: the properties the code must
//! satisfy, the shape that satisfies them, what one deviation costs, and
//! the library guarantee the cost follows from. A check contributes
//! only facts — which function, which site, how many, what text — and
//! the message is assembled mechanically by [`finding::detail`] from those
//! facts and the catalogue entry. No check writes a sentence of its own,
//! so a finding can never claim more than the catalogue and the fact base
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
//!
//! The module splits along responsibilities: [`catalogue`] holds the
//! forms and the derived facts licensing them, [`finding`] assembles what
//! a finding says, [`shipped_code`] decides which bodies a form speaks
//! about, and one module per check holds the decision it makes —
//! [`lock_run`], [`loop_write`], [`literal_format`] and
//! [`lost_write_error`]. A new output form is a new catalogue entry plus
//! one module beside those four.

mod catalogue;
mod finding;
mod literal_format;
mod lock_run;
mod loop_write;
mod lost_write_error;
mod shipped_code;

pub(super) use self::{
    literal_format::literal_format_insights, lock_run::lock_run_insights,
    loop_write::loop_write_insights, lost_write_error::lost_write_error_insights
};

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use std::path::PathBuf;

    use super::{lock_run_insights, loop_write_insights};
    use crate::{
        semantic::{behavior::Stream, workspace::Workspace},
        source::Source
    };

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
        let recorded = crate::semantic::behavior_with(&sources, Some(&crate::semantic::settling::standard_macros_over(&sources)))
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
            let found = loop_write_insights(&Workspace::resolved(&sources, &crate::semantic::settling::settled_over(&sources)));
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
            let found = lock_run_insights(&Workspace::resolved(&sources, &crate::semantic::settling::settled_over(&sources)));
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
}
