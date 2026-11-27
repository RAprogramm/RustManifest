// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What the compiler grants a type, stated per type rather than per
//! position.
//!
//! Whether a value moves when it is passed by value is not a property of
//! the place it is written at, it is a property of its type — so this row
//! is keyed by the type's name and not by a byte range, and one row serves
//! every position that type appears at.
//!
//! Answering it by name from a written list is what this replaces. A list
//! knows the types someone thought to write down; the compiler knows the
//! type in front of it, including one this workspace declared five minutes
//! ago and one reached through an alias.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// What the compiler grants one type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ResolvedProperty {
    /// Type the row speaks about, as the resolver renders it.
    pub named: String,
    /// Whether a value of this type is copied rather than moved when it
    /// is passed by value.
    pub copy: bool,
}
