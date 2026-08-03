// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a derivation yields: one property, the expression that proves
//! it, and the library's own sentence about it.
//!
//! The shapes here carry evidence and nothing else. Every field is
//! something read out of the library — a property name, a location, a
//! quote — so a consumer that holds one of these values holds the proof
//! with it and never has to trust a sentence written in this repository.

use std::path::PathBuf;

use rustmanifest_schema::Range;

/// One property of the standard output path, with the exact expression
/// that proves it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StdFact {
    /// Property name, as the forms refer to it. A property that holds
    /// per stream carries the stream after a colon.
    pub property: String,
    /// What the derivation read, spelled with the names it read there.
    pub observed: String,
    /// File the evidence lives in.
    pub file:     PathBuf,
    /// Byte range of the proving expression itself.
    pub range:    Range,
    /// The library's own documentation sentence for this property, read
    /// from the doc comment of the item that declares it. `None` when
    /// the library documents nothing about it, which is stated rather
    /// than filled in.
    pub doc:      Option<DocQuote>,
    /// One-based line of the proving expression.
    pub line:     usize,
    /// The library's own text at [`StdFact::range`], with runs of
    /// whitespace collapsed onto one line. This is the evidence a trace
    /// step shows: the source itself, never a restatement of it.
    pub snippet:  String
}

/// One sentence the library writes about itself, with its location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocQuote {
    /// The sentence, as written in the library's doc comment.
    pub text: String,
    /// File the sentence lives in.
    pub file: PathBuf,
    /// One-based line of the sentence.
    pub line: usize
}

/// Everything derived about the print path of the active toolchain.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StdOutputKnowledge {
    /// Facts in derivation order.
    pub facts: Vec<StdFact>
}

impl StdOutputKnowledge {
    /// Whether a property was derived.
    #[must_use]
    pub fn has(&self, property: &str) -> bool {
        self.facts.iter().any(|fact| fact.property == property)
    }

    /// The fact for one property, when derived.
    #[must_use]
    pub fn fact(&self, property: &str) -> Option<&StdFact> {
        self.facts.iter().find(|fact| fact.property == property)
    }
}
