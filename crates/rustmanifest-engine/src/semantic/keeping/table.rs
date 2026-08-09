// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! A store that exists only as a table the program's own queries name.
//!
//! A program serving straight over a database holds no collection
//! anywhere: its records live in rows, and the only place their home is
//! written is the SQL text itself. The table named there is a store on
//! the same terms as a declared field — the name is the author's, the
//! site is citable — and each verb is the statement's own kind: an
//! insert adds, an update changes, a delete takes out, a select reads.
//!
//! A statement that names no single table contributes no store and no
//! verb; its site stays on the query channel, so what this reading
//! skipped remains countable.
//!
//! A table named from a body that also touches a declared field store
//! is that store's far side — the outliving clause of the field's own
//! sentence already carries it — and a second sentence about the same
//! records would state one store twice. Such a table enters this
//! reading not at all.

use rustmanifest_schema::Range;

use super::verb::{Keeping, Verb};
use crate::semantic::{Statement, workspace::Workspace};

/// One table the workspace's own queries name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableStore {
    /// Table name as the first naming statement spells it.
    pub table:  String,
    /// Byte range of that statement's carrier.
    pub site:   Range,
    /// Source it was read from.
    pub source: usize
}

/// The key a table store is joined under, distinct from every
/// `Owner.field` key by its prefix.
#[must_use]
pub fn table_key(table: &str) -> String {
    format!("table:{table}")
}

/// Every table the workspace's queries name, first naming site cited,
/// sorted by name so two runs state them in one order.
#[must_use]
pub fn tables(workspace: &Workspace<'_>) -> Vec<TableStore> {
    let tied = tied_functions(workspace);
    let mut held: Vec<TableStore> = Vec::new();
    for fact in workspace.facts() {
        if fact.flags.test() || tied.contains(&fact.function) {
            continue;
        }
        for site in &fact.queries {
            let Some(named) = site.table.as_deref() else {
                continue;
            };
            if held.iter().any(|store| store.table == named) {
                continue;
            }
            held.push(TableStore {
                table:  named.to_owned(),
                site:   site.range,
                source: fact.source
            });
        }
    }
    held.sort_by(|left, right| left.table.cmp(&right.table));
    held
}

/// What every function does to every table its statements name, one
/// entry per proven kind, on the same terms as the field-store reading.
#[must_use]
pub fn table_keeping(workspace: &Workspace<'_>) -> Vec<Keeping> {
    let tied = tied_functions(workspace);
    let mut read = Vec::new();
    for fact in workspace.facts() {
        if fact.flags.test() || tied.contains(&fact.function) {
            continue;
        }
        for site in &fact.queries {
            let Some(named) = site.table.as_deref() else {
                continue;
            };
            let entry = Keeping {
                function: fact.function.clone(),
                store:    table_key(named),
                verb:     statement_verb(site.statement),
                site:     site.range,
                source:   fact.source
            };
            if !read.contains(&entry) {
                read.push(entry);
            }
        }
    }
    read
}

/// Every function whose body touches a declared field store, by name:
/// the functions whose statements speak of a store's far side rather
/// than of a store of their own.
fn tied_functions(workspace: &Workspace<'_>) -> Vec<String> {
    super::verb::keeping(workspace)
        .into_iter()
        .map(|kept| kept.function)
        .chain(
            super::outliving::outliving(workspace)
                .into_iter()
                .map(|carried| carried.function)
        )
        .collect()
}

/// The verb one statement kind performs, by that kind alone.
const fn statement_verb(statement: Statement) -> Verb {
    match statement {
        Statement::Inserts => Verb::Adds,
        Statement::Updates => Verb::Marks,
        Statement::Deletes => Verb::Drops,
        Statement::Selects => Verb::Reads
    }
}
