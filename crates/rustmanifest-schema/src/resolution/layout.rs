// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What one type weighs, as the compiler lays it out for the target it was
//! asked about.
//!
//! A written table of sizes states them for one target and says nothing
//! about being wrong on another: a pointer is eight bytes until the target
//! is a thirty-two-bit one. The compiler answers for the target in front
//! of it, and a type whose layout it declines to give — a generic
//! parameter, a type behind an unresolved path — is absent here rather
//! than estimated.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The layout the compiler gives one type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ResolvedLayout {
    /// Type the row speaks about, as the resolver renders it.
    pub named: String,
    /// Bytes a value of the type occupies.
    pub size:  u64,
    /// Byte alignment the type requires.
    pub align: u64
}
