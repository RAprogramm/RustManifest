// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! One observed choice among written alternatives.

use super::{
    super::{
        output::{Multiplicity, SiteGuard},
        value::{Condition, Provenance}
    },
    alternative::{Alternative, Closure}
};

/// One observed choice among written alternatives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    /// Where the value being chosen on came from.
    pub scrutinee:    Provenance,
    /// The alternatives, in the order the source writes them.
    pub alternatives: Vec<Alternative>,
    /// How the set of alternatives is closed.
    pub closure:      Closure,
    /// Byte range of the choosing construct.
    pub site:         rustmanifest_schema::Range,
    /// What stands between the site and unconditional execution.
    pub guard:        SiteGuard,
    /// What each construct standing over the site tests, outermost first.
    pub conditions:   Vec<Condition>,
    /// How many times it runs per call.
    pub multiplicity: Multiplicity
}
