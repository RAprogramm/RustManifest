// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Whether what a program keeps outlives the run that made it.
//!
//! A store held in memory is gone when the process ends. A method that
//! touches the store and, in the same body, reaches the file system
//! carries it across that boundary — and which way it carries is read
//! from the call that proved the effect, never from the method's name.
//!
//! Only the file system is read this way. A store carried over a socket
//! or into a database is carried by machinery this reading does not
//! follow, and saying nothing is the honest answer there.

use rustmanifest_schema::Range;

use super::verb::{Keeping, keeping};
use crate::semantic::{behavior::Effect, workspace::Workspace};

/// Which way a method carries a store across the end of the run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Puts what the store holds where the next run can read it.
    Saves,
    /// Fills the store from what an earlier run left.
    Loads
}

impl std::fmt::Display for Direction {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(match self {
            Self::Saves => "writes the records to a file",
            Self::Loads => "fills the records from a file"
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

/// Written spellings that put bytes where a later run can read them.
const WRITING: [&str; 6] = [
    "write",
    "write_all",
    "write_fmt",
    "create",
    "append",
    "create_new"
];

/// Every method that carries a store across the end of the run.
#[must_use]
pub fn outliving(workspace: &Workspace<'_>) -> Vec<Outliving> {
    let mut carried = Vec::new();
    for kept in filling(workspace) {
        let Some(fact) = workspace.facts().get(index_of(workspace, &kept)) else {
            continue;
        };
        for site in &fact.effect_sites {
            if site.effect != Effect::FileSystem {
                continue;
            }
            let named = site.proof.rsplit("::").next().unwrap_or(&site.proof);
            carried.push(Outliving {
                function:  kept.function.clone(),
                store:     kept.store.clone(),
                direction: if WRITING.contains(&named) {
                    Direction::Saves
                } else {
                    Direction::Loads
                },
                proof:     site.proof.clone(),
                site:      site.range,
                source:    kept.source
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
