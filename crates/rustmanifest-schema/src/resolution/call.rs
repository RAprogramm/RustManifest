// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! One call a resolver settled, and one field access.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{positioned::Positioned, signature::ResolvedMode};
use crate::Location;

/// One call a resolver settled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ResolvedCall {
    /// File and byte range of the call expression.
    pub at:       Location,
    /// Name of the function the call reaches.
    pub callee:   String,
    /// Type the call hands back, as the resolver renders it.
    pub returns:  String,
    /// Type of the receiver, where the call has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receiver: Option<String>,
    /// Full path of the function the call reaches, where the resolver
    /// settled one.
    ///
    /// The callee's own name is what a reading can already see written;
    /// which function it is, is not. Two types with a `new` share a name
    /// and never a path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reaches:  Option<String>,
    /// How the function the call reaches takes its receiver, where it has
    /// one.
    ///
    /// Whether a call changes what it is written on, and whether it ends
    /// that value's life, is the receiver's declared mode and nothing
    /// else. A written name cannot carry it: `replace` mutates a `Vec` in
    /// place and hands a `String` back untouched, and `take` moves out of
    /// an `Option` while leaving a `Vec` where it stood.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub takes:    Option<ResolvedMode>
}

impl Positioned for ResolvedCall {
    fn at(&self) -> &Location {
        &self.at
    }

    fn tiebreak(&self) -> &str {
        &self.callee
    }
}

/// One field access a resolver settled.
///
/// The engine reads a field by naming the type that holds it, and a type
/// outside the workspace is one the written text cannot name. This row
/// answers exactly that: which type the value being read from has, and
/// which type the field itself has.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ResolvedField {
    /// File and byte range of the field expression.
    pub at:       Location,
    /// Type of the value the field is read out of.
    pub owner:    String,
    /// Type the field itself has, as the resolver renders it.
    pub produced: String
}

impl Positioned for ResolvedField {
    fn at(&self) -> &Location {
        &self.at
    }

    fn tiebreak(&self) -> &str {
        &self.owner
    }
}
