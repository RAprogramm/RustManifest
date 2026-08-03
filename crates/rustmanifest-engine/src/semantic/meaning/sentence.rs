// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! One claim, one sentence.
//!
//! The sentence degrades field by field: the opening states how
//! certainly the write happens, the destination says it cannot be
//! identified rather than guessing one, the payload prints only what is
//! proven, and a weakened claim names every construct that weakened it.
//! An empty claim list says the recognizer matched nothing, which is not
//! the same statement as the program writing nothing.

use std::fmt::Write as _;

use super::{
    EntryMeaning, OutputClaim,
    payload::{describe, hole_name, reason_text, with_terminator},
    weakening::{chain_reason, site_reasons, weakening_of}
};
use crate::semantic::behavior::{Grade, Multiplicity, OUTPUT_SINK_SET, Payload, Segment, Stream};

impl EntryMeaning {
    /// Renders every claim as one sentence in the order the claims were
    /// recorded, each degraded exactly as far as its weakest field
    /// allows.
    ///
    /// An empty claim list means one thing only: the recognizer set of
    /// this build matched no write anywhere in the sources. It never
    /// means a write was found and set aside.
    #[must_use]
    pub fn sentences(&self) -> Vec<String> {
        if self.claims.is_empty() {
            return vec![format!(
                "no recognized write to a standard stream (recognizer set v{OUTPUT_SINK_SET}); \
                 this states the recognizer found none, not that the program writes nothing"
            )];
        }
        self.claims.iter().map(OutputClaim::sentence).collect()
    }
}

impl OutputClaim {
    /// What stands between this claim and certainty, or `None` when
    /// nothing does.
    #[must_use]
    pub fn weakening(&self) -> Option<String> {
        weakening_of(self)
    }

    /// What the sentence still has to name after its opening words.
    ///
    /// The opening of an unreached claim already states the call chain,
    /// so repeating it would say the same thing twice; everything else
    /// the claim carries is named at every grade below certainty,
    /// because a claim that is dead or unreached has a cause exactly as
    /// a possible one does.
    fn stated_weakening(&self) -> Option<String> {
        let reasons = match self.execution {
            Grade::Certain => Vec::new(),
            Grade::Unreached => site_reasons(&self.write.guard),
            Grade::Possible | Grade::Dead => chain_reason(self.reach)
                .into_iter()
                .map(ToOwned::to_owned)
                .chain(site_reasons(&self.write.guard))
                .collect()
        };
        (!reasons.is_empty()).then(|| reasons.join(" and "))
    }

    /// The text this claim proves, rendered without ever standing in a
    /// value it cannot see: holes keep their names, an unreadable
    /// payload states its reason.
    #[must_use]
    pub fn payload_text(&self) -> String {
        match &self.write.payload {
            Payload::Empty => "<nothing>".to_owned(),
            Payload::Literal(text) => format!("{:?}", with_terminator(text, self.write.newline)),
            Payload::Template(segments) => {
                let shape: String = segments
                    .iter()
                    .map(|segment| match segment {
                        Segment::Text(text) => text.clone(),
                        Segment::Hole(hole) => format!("<{}>", hole_name(hole))
                    })
                    .collect();
                format!("{:?}", with_terminator(&shape, self.write.newline))
            }
            Payload::Opaque(reason) => format!("<unreadable: {}>", reason_text(*reason))
        }
    }

    /// The sentence this claim licenses.
    #[must_use]
    pub fn sentence(&self) -> String {
        let opening = match self.execution {
            Grade::Certain => "the process unconditionally writes",
            Grade::Possible => "the process may write",
            Grade::Dead => "the process contains a write that can never run:",
            Grade::Unreached => "no path from the entry point reaches a write of"
        };
        let destination = match self.write.stream {
            Stream::Stdout => "its standard output stream",
            Stream::Stderr => "its standard error stream",
            Stream::CallerSupplied => "a stream the caller supplies",
            Stream::Unknown => "a stream this analysis cannot identify"
        };
        let mut sentence = format!(
            "{opening} {} to {destination}",
            describe(&self.write.payload, self.write.newline)
        );
        if self.write.multiplicity == Multiplicity::Unbounded {
            sentence.push_str(", once per iteration of the loop around it");
        }
        if let Some(reason) = self.stated_weakening() {
            let _ = write!(sentence, ", weakened by {reason}");
        }
        let _ = write!(
            sentence,
            " [{} in `{}`, attempted: the library can divert or fail the write]",
            self.file, self.function
        );
        sentence
    }
}
