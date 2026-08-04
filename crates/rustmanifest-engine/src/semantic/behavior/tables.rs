// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Workspace typing tables shared by every function's fact extraction:
//! what each field of each struct holds, what each function returns, what
//! each named string constant stands for, and which crate every source
//! belongs to.
//!
//! The tables that have to walk the whole workspace to be built sit in
//! submodules beside this file; crate identity reads a path and stays
//! here.

mod constant;
mod field;
mod function_return;

use super::{returned::Returned, scoped::Scoped};
use crate::source::Source;

/// Workspace typing tables shared by every function's fact extraction.
pub(super) struct TypeTables {
    /// Function names to their return types, scoped by source.
    pub(super) returns:     Scoped<Returned>,
    /// `Type.field` keys to field type names, scoped by source.
    pub(super) fields:      Scoped<String>,
    /// `Type.field` keys of collection fields to element types.
    pub(super) field_elems: Scoped<String>,
    /// `Type.field` keys of single-generic wrapper fields to the
    /// wrapped inner type.
    pub(super) field_wraps: Scoped<String>,
    /// Names of string constants to the literal each is written as.
    pub(super) constants:   Scoped<String>,
    /// Crate identity of every source, for crate-proximity resolution.
    pub(super) crates:      Vec<String>
}

impl TypeTables {
    /// Reads every typing table off `sources` in one pass each.
    pub(super) fn of(sources: &[Source]) -> Self {
        let (fields, field_elems, field_wraps) = field::field_tables(sources);
        Self {
            returns: function_return::return_types(sources),
            fields,
            field_elems,
            field_wraps,
            constants: constant::constant_texts(sources),
            crates: crate_ids(sources)
        }
    }
}

/// Crate identity of every source, from the path component preceding
/// its `src` directory.
fn crate_ids(sources: &[Source]) -> Vec<String> {
    sources
        .iter()
        .map(|source| {
            let path = source.path().to_string_lossy().replace('\\', "/");
            let components: Vec<&str> = path.split('/').filter(|c| !c.is_empty()).collect();
            components
                .iter()
                .position(|component| *component == "src")
                .and_then(|position| position.checked_sub(1))
                .and_then(|index| components.get(index))
                .map(|name| (*name).to_owned())
                .unwrap_or_default()
        })
        .collect()
}
