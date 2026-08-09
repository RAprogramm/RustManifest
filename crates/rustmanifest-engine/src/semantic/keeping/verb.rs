// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a method does to the store its type holds.
//!
//! A call counts only where it was made on the collection the store
//! declares: a push into a list built inside the method is that list's
//! business, and reading it as the store's would call every method that
//! gathers an answer an addition.
//!
//! Four verbs, each read from a fact already recorded and none from a
//! name: a method putting a value into the collection adds, one changing
//! a field of a record taken out of it marks, one taking a value out
//! drops, and one that only reads it out lists. A method touching the
//! collection in none of those ways gets no verb at all — silence is
//! "no verb proven", never "does nothing".

use rustmanifest_schema::Range;

use super::store::Store;
use crate::semantic::workspace::Workspace;

/// What one method does to a store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verb {
    /// Puts a record into the collection.
    Adds,
    /// Changes a field of a record held in it.
    Marks,
    /// Takes a record out of it.
    Drops,
    /// Reads the records out without changing them.
    Reads
}

impl std::fmt::Display for Verb {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(match self {
            Self::Adds => "adds a record",
            Self::Marks => "changes a record",
            Self::Drops => "takes a record out",
            Self::Reads => "reads the records"
        })
    }
}

/// One method and what it does to one store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keeping {
    /// Qualified name of the method.
    pub function: String,
    /// The store it touches, as `Owner.field`.
    pub store:    String,
    /// What it does to it.
    pub verb:     Verb,
    /// Byte range of the method's own name.
    pub site:     Range,
    /// Source the method was read from.
    pub source:   usize
}

/// What every method of the workspace does to every store it touches.
///
/// A `#[test]` function is not one of the program's own methods: what
/// it does to a store it does to a fixture of its own making, and
/// counting it would put test names into the sentence about the
/// program.
#[must_use]
pub fn keeping(workspace: &Workspace<'_>) -> Vec<Keeping> {
    let held = super::store::stores(workspace);
    let mut read = Vec::new();
    for fact in workspace.facts() {
        if fact.flags.test() {
            continue;
        }
        for store in &held {
            let key = format!("{}.{}", store.owner, store.field);
            let Some(verb) = verb_of(fact, store, &key) else {
                continue;
            };
            read.push(Keeping {
                function: fact.function.clone(),
                store: key,
                verb,
                site: fact.range,
                source: fact.source
            });
        }
    }
    read
}

/// The verb one function performs on one store, where it performs one.
fn verb_of(
    fact: &crate::semantic::behavior::FunctionFacts,
    store: &Store,
    key: &str
) -> Option<Verb> {
    let named = key.to_owned();
    let touches = fact.reads.contains(&named)
        || fact.writes.contains(&named)
        || fact.builds.contains(&named);
    if !touches {
        return None;
    }
    let called = |named: &str| {
        let spelling = format!("{}::{named}", store.shape);
        fact.call_sites
            .iter()
            .any(|site| site.candidate == spelling)
    };
    if called("remove") {
        return Some(Verb::Drops);
    }
    if called("insert") || called("push") {
        return Some(Verb::Adds);
    }
    let record_field = format!("{}.", store.record);
    if fact
        .writes
        .iter()
        .any(|write| write.starts_with(&record_field))
    {
        return Some(Verb::Marks);
    }
    reading_only(fact, &named)
}

/// A read proven by its own channel, and silence for every other touch.
///
/// The reads channel recording the field is the proof that the method
/// reads it. A touch recorded only as a write of the field itself — a
/// method assigning a fresh collection over it, or folding another one
/// in — proves none of the four verbs, and the answer there is silence:
/// calling a method that replaces every record a reader would state a
/// verb the facts never proved.
fn reading_only(fact: &crate::semantic::behavior::FunctionFacts, key: &str) -> Option<Verb> {
    fact.reads
        .iter()
        .any(|read| read == key)
        .then_some(Verb::Reads)
}
