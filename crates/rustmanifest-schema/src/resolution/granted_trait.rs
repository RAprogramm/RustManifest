// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Which trait a type carries, stated per type rather than per position.
//!
//! Whether a type has an ability is not a property of any place it is
//! written at, so the row is keyed by the type's name. It covers what a
//! written `impl` says and what a derive generates alike: to the compiler
//! both are the same answer, and a reading of the source has to know every
//! derive's expansion to agree.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One trait one type carries.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ResolvedGrant {
    /// Type the row speaks about, as the resolver renders it.
    pub named:      String,
    /// Trait that type carries, by the name it is declared under.
    pub trait_name: String
}
