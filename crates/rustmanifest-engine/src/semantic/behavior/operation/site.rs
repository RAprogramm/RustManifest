// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! One observed operation over values.

use super::{
    super::{
        output::{Multiplicity, SiteGuard},
        value::{Condition, Disposal, Provenance}
    },
    guarding::Guarding,
    kind::Operation
};

/// One observed operation over values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationSite {
    /// What the operation does.
    pub operation:    Operation,
    /// Where each operand came from, in written order.
    pub operands:     Vec<Provenance>,
    /// The type it produces, where the source names it.
    pub produced:     Option<String>,
    /// What the construct does with the failure it can have.
    pub guarding:     Guarding,
    /// What the site does with a failure the operation reports.
    pub disposal:     Disposal,
    /// Byte range of the operating construct.
    pub site:         rustmanifest_schema::Range,
    /// What stands between the site and unconditional execution.
    pub guard:        SiteGuard,
    /// What each construct standing over the site tests, outermost first.
    pub conditions:   Vec<Condition>,
    /// How many times it runs per call.
    pub multiplicity: Multiplicity
}
