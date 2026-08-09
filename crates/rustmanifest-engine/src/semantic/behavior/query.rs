// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! One written database statement a body carries, as data.
//!
//! The statement's kind and the table it names are both read off the
//! author's own SQL text where the classifier proves them. The table is
//! optional on purpose: a statement whose kind is proven but whose
//! table is not — a join, a subquery — is still an event worth a
//! record, and dropping it would lose the very count an honesty check
//! needs.

use std::fmt;

use rustmanifest_schema::Range;

/// The kind of one data statement, read from its first keyword.
///
/// Only the four kinds that touch rows are named: a DDL statement
/// proves a database effect but speaks of shape, not of records, and
/// stays out of this reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Statement {
    /// `SELECT`: reads rows out.
    Selects,
    /// `INSERT`: puts a row in.
    Inserts,
    /// `UPDATE`: changes rows in place.
    Updates,
    /// `DELETE`: takes rows out.
    Deletes
}

impl fmt::Display for Statement {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Selects => "select",
            Self::Inserts => "insert",
            Self::Updates => "update",
            Self::Deletes => "delete"
        })
    }
}

/// One written statement site: what it does, which table it names, and
/// where it stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuerySite {
    /// The kind its first keyword proves.
    pub statement: Statement,
    /// The table the statement's own grammar slot names, where a single
    /// plain name provably fills it.
    pub table:     Option<String>,
    /// Byte range of the written statement's carrier.
    pub range:     Range
}
