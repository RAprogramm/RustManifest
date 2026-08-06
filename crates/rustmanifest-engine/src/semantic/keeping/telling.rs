// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Whether what a program prints is what it keeps.
//!
//! A write whose position is filled by a value that came out of the
//! store is the program telling someone about its records. The link is
//! the composed origin the meaning tier already derived: a term naming
//! the store's own field, at any depth, is that store showing through.
//!
//! A write filled from anywhere else is not about the records, however
//! close it stands to them.

use rustmanifest_schema::Range;

use super::store::stores;
use crate::semantic::{ComposedOrigin, workspace::Workspace};

/// One write that shows what the program keeps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Telling {
    /// Function holding the write.
    pub function: String,
    /// The store it shows, as `Owner.field`.
    pub store:    String,
    /// The position of the write whose value came out of the store.
    pub position: String,
    /// Byte range of the writing construct.
    pub site:     Range
}

/// Every write whose value came out of a store the program keeps.
#[must_use]
pub fn telling(workspace: &Workspace<'_>) -> Vec<Telling> {
    let held = stores(workspace);
    let mut told = Vec::new();
    for entry in crate::semantic::program_meaning(workspace) {
        for claim in &entry.claims {
            for (position, origin) in &claim.sources {
                for store in &held {
                    let key = format!("{}.{}", store.owner, store.field);
                    if !shows(origin, &key) {
                        continue;
                    }
                    told.push(Telling {
                        function: claim.function.clone(),
                        store:    key,
                        position: position.clone(),
                        site:     claim.write.site
                    });
                }
            }
        }
    }
    told
}

/// Whether a composed term names one store's field anywhere inside it.
fn shows(origin: &ComposedOrigin, key: &str) -> bool {
    match origin {
        ComposedOrigin::Field(named) => named == key,
        ComposedOrigin::Computed {
            operands, ..
        } => operands.iter().any(|operand| shows(operand, key)),
        ComposedOrigin::Choice(alternatives) => alternatives
            .iter()
            .any(|alternative| shows(alternative, key)),
        ComposedOrigin::Call {
            arguments,
            carried,
            ..
        } => arguments.iter().any(|argument| shows(argument, key)) || shows(carried, key),
        _ => false
    }
}
