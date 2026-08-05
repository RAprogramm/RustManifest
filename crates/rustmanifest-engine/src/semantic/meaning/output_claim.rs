// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! One statement about what the program writes, and where the value
//! filling each of its named positions came from.

use super::super::{
    behavior::{Grade, HoleSource, OutputWrite, Payload, Segment},
    summary::ComposedOrigin
};

/// One statement about what the program writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputClaim {
    /// Function holding the write site.
    pub function:  String,
    /// File the site lives in.
    pub file:      String,
    /// The recorded write.
    pub write:     OutputWrite,
    /// How certainly the owning function is reached from the entry.
    ///
    /// [`Grade::Unreached`] is a claim of its own — the call graph found
    /// no path from this entry to the site — and is stated rather than
    /// dropped, so an empty claim list means the recognizer found no
    /// write at all and nothing else.
    pub reach:     Grade,
    /// How certainly the site runs, both parts composed.
    pub execution: Grade,
    /// Where the value filling each named position of the payload came
    /// from, with every reference followed and every call restated.
    ///
    /// One entry per named position of the payload. A term the reading
    /// could not settle is kept and says which question it failed on:
    /// dropping it would leave the claim silent about a position the
    /// program certainly fills.
    pub sources:   Vec<(String, ComposedOrigin)>,
    /// Version of the recognizer list that produced this claim.
    pub sink_set:  u32
}

/// Where the value filling each named position of one write came from.
pub(super) fn hole_sources(
    composed: &crate::semantic::summary::Composition<'_>,
    fact: usize,
    write: &OutputWrite
) -> Vec<(String, ComposedOrigin)> {
    let Payload::Template(segments) = &write.payload else {
        return Vec::new();
    };
    segments
        .iter()
        .filter_map(|segment| match segment {
            Segment::Hole(hole) => {
                let named = match &hole.source {
                    HoleSource::Named(name) => name.clone(),
                    _ => return None
                };
                Some((named, composed.of(fact, &hole.origin)))
            }
            Segment::Text(_) => None
        })
        .collect()
}
