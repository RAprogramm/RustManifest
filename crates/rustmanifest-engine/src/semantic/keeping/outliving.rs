// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Whether what a program keeps outlives the run that made it.
//!
//! A store held in memory is gone when the process ends. A method that
//! touches the store and, in the same body, reaches the file system or
//! the database carries it across that boundary — and which way it
//! carries is read from the call that proved the effect, never from the
//! method's name. For the database the effect itself already says the
//! way: a statement proven to mutate persists, a query proven to read
//! restores.
//!
//! Only those two boundaries are read. A store carried over a socket is
//! carried by machinery this reading does not follow, and saying
//! nothing is the honest answer there.

use rustmanifest_schema::Range;

use super::verb::{Keeping, keeping};
use crate::semantic::{behavior::Effect, workspace::Workspace};

/// Which way a method carries a store across the end of the run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Puts what the store holds where the next run can read it.
    Saves,
    /// Fills the store from what an earlier run left.
    Loads,
    /// Puts what the store holds into the database.
    Persists,
    /// Fills the store from rows the database holds.
    Restores
}

impl std::fmt::Display for Direction {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(match self {
            Self::Saves => "writes the records to a file",
            Self::Loads => "fills the records from a file",
            Self::Persists => "writes the records to the database",
            Self::Restores => "fills the records from the database"
        })
    }
}

/// One method carrying one store across the end of the run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outliving {
    /// Qualified name of the method.
    pub function:  String,
    /// The store it carries, as `Owner.field`.
    pub store:     String,
    /// Which way it carries it.
    pub direction: Direction,
    /// The written call that proved the file effect.
    pub proof:     String,
    /// Byte range of that call.
    pub site:      Range,
    /// Source the method was read from.
    pub source:    usize
}

/// The trait whose bodies put bytes on a sink, so a call reaching one
/// writes whatever it is called on.
const WRITING_TRAIT: &str = "std::io::Write";

/// The library bodies that put bytes where a later run can read them
/// without going through the writing trait, each named by the item the
/// call reaches.
///
/// Six written spellings answered this before, and answered for any
/// namesake: a workspace method called `create` was read as a save.
const WRITING_BODIES: [&str; 4] = [
    "std::fs::File::create",
    "std::fs::File::create_new",
    "std::fs::write",
    "std::fs::OpenOptions::append"
];

/// Whether the body a site proves the effect with puts bytes out.
fn writes_out(proof: &str) -> bool {
    proof.starts_with(&format!("{WRITING_TRAIT}::"))
        || WRITING_BODIES.contains(&proof)
}

/// Every method that carries a store across the end of the run.
#[must_use]
pub fn outliving(workspace: &Workspace<'_>) -> Vec<Outliving> {
    let mut carried = Vec::new();
    for kept in filling(workspace) {
        let Some(fact) = workspace.facts().get(index_of(workspace, &kept)) else {
            continue;
        };
        for site in &fact.effect_sites {
            let direction = match site.effect {
                Effect::FileSystem if writes_out(&site.proof) => Direction::Saves,
                Effect::FileSystem => Direction::Loads,
                Effect::DatabaseWrite => Direction::Persists,
                Effect::DatabaseRead => Direction::Restores,
                _ => continue
            };
            carried.push(Outliving {
                function: kept.function.clone(),
                store: kept.store.clone(),
                direction,
                proof: site.proof.clone(),
                site: site.range,
                source: kept.source
            });
        }
    }
    carried
}

/// Every method that touches a store, whether by naming its field or by
/// building the value that holds it.
///
/// A constructor filling a fresh value from a file names no field at
/// all — the collection is a local until the moment it is moved in — and
/// reading only the named touches lost exactly the method that brings a
/// store into being.
fn filling(workspace: &Workspace<'_>) -> Vec<Keeping> {
    let mut touched = keeping(workspace);
    for store in super::store::stores(workspace) {
        let key = format!("{}.{}", store.owner, store.field);
        for fact in workspace.facts() {
            let builds = fact
                .constructs
                .iter()
                .any(|built| *built == store.owner || built == "Self")
                && fact.function.starts_with(&format!("{}::", store.owner));
            if !builds || touched.iter().any(|kept| kept.function == fact.function) {
                continue;
            }
            touched.push(Keeping {
                function: fact.function.clone(),
                store:    key.clone(),
                verb:     super::verb::Verb::Adds,
                site:     fact.range,
                source:   fact.source
            });
        }
    }
    touched
}

/// Index of the fact one keeping was read from.
fn index_of(workspace: &Workspace<'_>, kept: &Keeping) -> usize {
    workspace
        .facts()
        .iter()
        .position(|fact| {
            let same_name = fact.function == kept.function;
            same_name && fact.range == kept.site
        })
        .unwrap_or(usize::MAX)
}
