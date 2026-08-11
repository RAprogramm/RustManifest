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
    /// The type the carrier itself writes for the rows — the macro's
    /// leading type argument or the call's turbofish — by its last
    /// segment. A carrier writing no type names none: inference is the
    /// compiler's knowledge, not the source's.
    pub record:    Option<String>,
    /// Plain names written as the statement's own arguments after the
    /// SQL, in written order. A bound expression that is not a plain
    /// name is not listed: what a computed argument holds is the value
    /// tier's question, and this channel records only what the site
    /// spells.
    pub binds:     Vec<String>,
    /// Byte range of the written statement's carrier.
    pub range:     Range
}

/// How many database effects the statement channel accounts for.
///
/// Three counters, none a claim: sites proving a database effect, sites
/// whose statement kind the channel proved, and among those the sites
/// naming their table. A statement built at run time proves the effect
/// and nothing further, so the first number exceeding the second is the
/// channel's own blind spot, stated as a number rather than left to be
/// discovered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatementCoverage {
    /// Sites proving a database effect, DDL and run-time statements
    /// included.
    pub effect_sites: usize,
    /// Statement sites whose kind the first keyword proved.
    pub kinds_proven: usize,
    /// Of those, the sites whose table one plain grammar slot names.
    pub tables_named: usize
}

/// Counts the statement channel against the database effects it covers.
#[must_use]
pub fn statement_coverage(facts: &[super::FunctionFacts]) -> StatementCoverage {
    let mut counted = StatementCoverage {
        effect_sites: 0,
        kinds_proven: 0,
        tables_named: 0
    };
    for fact in facts {
        counted.effect_sites += fact
            .effect_sites
            .iter()
            .filter(|site| {
                matches!(
                    site.effect,
                    super::Effect::DatabaseRead
                        | super::Effect::DatabaseWrite
                        | super::Effect::Database
                )
            })
            .count();
        counted.kinds_proven += fact.queries.len();
        counted.tables_named += fact
            .queries
            .iter()
            .filter(|site| site.table.is_some())
            .count();
    }
    counted
}
