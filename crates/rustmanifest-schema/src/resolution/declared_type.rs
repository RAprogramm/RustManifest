// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a declared type is made of, as the compiler reads its definition.
//!
//! A field's type is written in the definition, and reading it there is a
//! walk over syntax that has to resolve aliases, `Self`, generics and
//! imports before it means anything. The compiler has already done all of
//! that, and states the field's type in full.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Location;

/// One field of a declared type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ResolvedFieldDecl {
    /// Name the field is declared under, its position for a tuple field.
    pub name:  String,
    /// Type the field has, as the resolver renders it.
    pub named: String
}

/// One type the workspace declares.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ResolvedTypeDecl {
    /// File and byte range of the type's name.
    pub at:     Location,
    /// Name the type is declared under.
    pub named:  String,
    /// Fields it declares, in written order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<ResolvedFieldDecl>
}
