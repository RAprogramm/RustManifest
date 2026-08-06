// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a program holds: a field whose type is a collection of records
//! the workspace itself declares.
//!
//! A collection of a type another crate declares is not a store of this
//! program's records — it may be anything at all — so only the types the
//! workspace declares are read this way. The record must be a structure
//! with fields: a collection of enumerations is a list of choices, not a
//! set of records, and the distinction is the declaration's, not a
//! reader's.

use rustmanifest_schema::Range;

use crate::semantic::{TypeKind, workspace::Workspace};

/// One collection of records a type holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Store {
    /// Type declaring the field.
    pub owner:  String,
    /// Field holding the collection.
    pub field:  String,
    /// Collection type as written, by its last segment.
    pub shape:  String,
    /// Type of the records inside it.
    pub record: String,
    /// Byte range of the field's declaration.
    pub site:   Range,
    /// Source the declaration was read from.
    pub source: usize
}

/// The collections of workspace records every workspace type holds.
///
/// Sorted by owner and then by field, so two runs over one workspace
/// state them in one order.
#[must_use]
pub fn stores(workspace: &Workspace<'_>) -> Vec<Store> {
    let graph = workspace.types();
    let mut held = Vec::new();
    for node in graph.types() {
        if node.kind != TypeKind::Struct {
            continue;
        }
        for field in &node.fields {
            let Some(record) = graph
                .types()
                .iter()
                .find(|candidate| candidate.kind == TypeKind::Struct && kept(field, candidate))
            else {
                continue;
            };
            held.push(Store {
                owner:  node.name.clone(),
                field:  field.name.clone(),
                shape:  field.type_name.clone(),
                record: record.name.clone(),
                site:   node.range,
                source: node.source
            });
        }
    }
    held.sort_by(|left, right| {
        left.owner
            .cmp(&right.owner)
            .then(left.field.cmp(&right.field))
    });
    held
}

/// Whether a field holds a collection of `record`.
///
/// The element type is read off the written type argument the field
/// table already keeps; a field whose type carries no argument holds no
/// collection and is not a store however it is named.
fn kept(field: &crate::semantic::graph::FieldNode, record: &crate::semantic::TypeNode) -> bool {
    field
        .element
        .as_deref()
        .is_some_and(|element| element == record.name)
}
