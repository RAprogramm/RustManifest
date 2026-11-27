// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What one written path names, settled to the item it reaches.
//!
//! A written path is not an identity. Two crates may both declare `Error`,
//! an import may rename one of them, and a local alias may stand for either.
//! Keying a fact by the last segment of what was typed collapses all three
//! into one bucket, and no amount of care further down recovers the
//! difference. This row is the identity: the full path of the item the name
//! actually reaches, and what kind of item that is.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::positioned::Positioned;
use crate::Location;

/// What one written path names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ResolvedPath {
    /// File and byte range of the written path.
    pub at:        Location,
    /// Full path of the item the written one reaches, crate root first.
    pub names:     String,
    /// What kind of item that is: `function`, `struct`, `enum`, `trait`,
    /// `module`, `const`, `static`, `type-alias`, `variant`, `field`,
    /// `macro`, `local`, or `self-type`.
    pub kind:      String,
    /// Crate the item is declared in, so a namesake from another crate is
    /// distinguishable without parsing the path.
    pub declaring: String
}

impl Positioned for ResolvedPath {
    fn at(&self) -> &Location {
        &self.at
    }

    fn tiebreak(&self) -> &str {
        &self.names
    }
}
