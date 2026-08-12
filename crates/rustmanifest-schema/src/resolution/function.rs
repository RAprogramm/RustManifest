// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! One function the workspace declares, named the way its crate names it.
//!
//! A body is identified here by the place it is written at, and named by
//! the full path that reaches it. Two functions called `new` in two types
//! are two rows; a reading that keys facts by the written name has one
//! bucket for both and has to guess which is meant.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::positioned::Positioned;
use crate::Location;

/// One declared function.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ResolvedFunction {
    /// File and byte range of the whole declaration.
    pub at:    Location,
    /// Full path that reaches the function, crate name first.
    pub names: String
}

impl Positioned for ResolvedFunction {
    fn at(&self) -> &Location {
        &self.at
    }

    fn tiebreak(&self) -> &str {
        &self.names
    }
}
