// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The type of one expression, settled where it is written.
//!
//! A call and a field access each get a row of their own because the engine
//! asks a further question about them — which function was reached, which
//! type holds the field. Every other expression is asked one question only:
//! what type does it have. One row shape answers it for all of them, which is
//! what lets a reader stop matching on how an expression was spelled.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::positioned::Positioned;
use crate::Location;

/// The type one expression has, as the resolver renders it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ResolvedType {
    /// File and byte range of the expression.
    pub at:       Location,
    /// Type the expression has, rendered the way the resolver displays it.
    pub rendered: String,
    /// Full path of the type an item declares it as, spelled from the
    /// crate that declares it.
    ///
    /// The rendering names the type; this says which one it is. Two
    /// crates each declaring a `Stdout` render the same and never share
    /// a path, and a reading keyed by the rendering credits one with the
    /// other's guarantees.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared: Option<String>
}

impl Positioned for ResolvedType {
    fn at(&self) -> &Location {
        &self.at
    }

    fn tiebreak(&self) -> &str {
        &self.rendered
    }
}
