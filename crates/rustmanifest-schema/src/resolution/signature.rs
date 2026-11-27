// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What one function takes and what it hands back, as the compiler reads
//! its signature.
//!
//! A reading of the written text answers this only where the text spells
//! it out. The compiler answers it whatever the source elides: a receiver
//! whose type is the impl's, a parameter typed through an alias, a
//! lifetime the source never wrote.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::positioned::Positioned;
use crate::Location;

/// How a function takes one of its parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ResolvedMode {
    /// Taken by value; passing it moves or copies it.
    Owned,
    /// Taken by shared borrow.
    Borrowed,
    /// Taken by exclusive borrow.
    BorrowedMut
}

/// One parameter of a function.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ResolvedParam {
    /// Name the parameter is bound to, `self` for a receiver.
    pub name:  String,
    /// Type the parameter has, as the resolver renders it.
    pub named: String,
    /// How the function takes it.
    pub mode:  ResolvedMode
}

/// The signature of one declared function.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ResolvedSignature {
    /// File and byte range of the function's name.
    pub at:      Location,
    /// Parameters in written order, the receiver first where there is one.
    pub params:  Vec<ResolvedParam>,
    /// Type the function hands back.
    pub returns: String
}

impl Positioned for ResolvedSignature {
    fn at(&self) -> &Location {
        &self.at
    }

    fn tiebreak(&self) -> &str {
        &self.returns
    }
}
