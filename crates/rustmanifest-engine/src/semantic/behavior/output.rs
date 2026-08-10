// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Observable output: what a function writes, to which stream, and under
//! which conditions.
//!
//! The effect list answers "something is printed" and cannot answer more:
//! it is sorted and deduplicated, so it carries neither payload, nor
//! order, nor the branch a write sits under. This module holds the
//! parallel channel that does — one record per write site, in body
//! order, never deduplicated, each carrying what the analysis can prove
//! and nothing beyond it.
//!
//! Honesty rules encoded in the types themselves: a payload whose text
//! is not fully proven cannot be rendered as a string
//! ([`Payload::fully_known`] refuses), a stream that cannot be
//! identified is recorded as such rather than omitted, and the version
//! of the recognizer list is stamped into every claim so absence reads
//! as "the recognizer found none", never as "there are none".
//!
//! The record and every reading it carries sit in a submodule of their
//! own beside this file: the stream, the payload and its recovery, the
//! grade, the guards behind it, and how often it repeats.

mod acquisition;
mod conduit;
mod grade;
mod guard;
mod multiplicity;
mod payload;
mod stream;
mod write;

pub(super) use self::payload::{argument_payload, literal_bytes_payload, macro_payload};
pub use self::{
    acquisition::Acquisition,
    conduit::Conduit,
    grade::Grade,
    guard::{Guard, SiteGuard},
    multiplicity::Multiplicity,
    payload::{Hole, HoleSource, OpaqueReason, Payload, Segment},
    stream::Stream,
    write::{Buffering, Newline, OutputWrite}
};

/// Version of the recognized write-sink list. Stamped into claims so a
/// negative result states which recognizer produced it.
pub const OUTPUT_SINK_SET: u32 = 3;
