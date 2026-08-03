// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Mechanical assembly of what a finding says: its message, its trace,
//! and the memory cost it states.
//!
//! A check hands over facts — which function, which site, how many, what
//! text — and a catalogue entry; the sentences are put together here in a
//! fixed shape. No check writes prose of its own, so a finding can never
//! claim more than the catalogue and the fact base together license.

use rustmanifest_schema::TraceStep;

use super::catalogue::Form;
use crate::semantic::std_derive::{StdFact, StdOutputKnowledge};

/// Assembles the message of one finding from facts and a catalogue
/// entry, in a fixed shape no check may deviate from.
pub(super) fn detail(form: &Form, subject: &str, observed: &str, times: u32) -> String {
    format!(
        "`{subject}` — {observed}\n    required (methodology): {}\n    cost: {times} {}{}",
        form.canonical,
        form.unit,
        if times == 1 { "" } else { "s" }
    )
}

/// Assembles the trace: the observed fact, the library's own source text
/// at the location the property was derived from, and the library's own
/// sentence about that property when it writes one.
pub(super) fn trace(observed: &str, evidence: &str, derived: &StdFact) -> Vec<TraceStep> {
    let mut steps = vec![
        TraceStep {
            fact:   observed.to_owned(),
            reason: evidence.to_owned()
        },
        TraceStep {
            fact:   format!("`{}`", derived.snippet),
            reason: format!("{}:{}", derived.file.display(), derived.line)
        },
    ];
    if let Some(quote) = &derived.doc {
        steps.push(TraceStep {
            fact:   quote.text.clone(),
            reason: format!("{}:{}", quote.file.display(), quote.line)
        });
    }
    steps
}

/// What the write costs in memory, stated as a chain of counted facts.
///
/// The payload's bytes are static: proven text lives in the binary and
/// is neither built nor copied at run time. The stream's buffer is heap,
/// and its size is the library's own literal. When the payload ends the
/// line and the stream buffers by line, that buffer is handed to the
/// operating system on this very write, so it amortizes nothing here —
/// which is the fact a reader needs to judge whether the buffered path
/// earns its allocation.
pub(super) fn memory_steps(
    text: &str,
    write: &crate::semantic::OutputWrite,
    library: &StdOutputKnowledge
) -> Vec<TraceStep> {
    let mut steps = vec![TraceStep {
        fact:   format!("{} static byte(s)", text.len()),
        reason: "the proven text lives in the binary, not on the stack or the heap".to_owned()
    }];
    let Some(capacity) = library.fact("buffer-capacity") else {
        return steps;
    };
    steps.push(TraceStep {
        fact:   format!(
            "{} heap byte(s) on the first write to the stream",
            capacity.observed
        ),
        reason: format!("{}:{}", capacity.file.display(), capacity.line)
    });
    let buffered = library.has("line-buffered:stdout") || library.has("line-buffered:stderr");
    if buffered && write.newline == crate::semantic::Newline::Appended {
        steps.push(TraceStep {
            fact:   "the buffer is handed to the operating system on this write".to_owned(),
            reason: "the payload ends the line and the stream buffers by line, so the buffer \
                     holds nothing across writes here"
                .to_owned()
        });
    }
    steps
}
