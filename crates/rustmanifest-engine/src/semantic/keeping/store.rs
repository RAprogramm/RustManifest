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

use crate::semantic::{TypeKind, behavior::Effect, workspace::Workspace};

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
    /// Type a lookup into it is written with, where it is a map.
    pub key:    Option<String>,
    /// Whether the owner declares itself a wire shape: a serialization
    /// derive written on the type says its values are made to leave or
    /// enter the process, and the declaration is the type's own.
    pub wire:   bool,
    /// Byte range of the field's declaration.
    pub site:   Range,
    /// Source the declaration was read from.
    pub source: usize
}

/// The trait a shape carries exactly when it takes many values in.
///
/// A field of any other shape — an `Option`, a `Result`, a domain
/// wrapper — holds one value or none, and one value is not a set of
/// records however its type is spelled. The first corpus run made the
/// case: an optional error inside a response envelope read as "keeps
/// error records" until the shape was required to be a collection.
/// Eight written container names stood for the answer before, and stood
/// for it only for the eight somebody listed.
const TAKES_MANY: &str = "Extend";

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
            let Some(element) = field.element.as_deref() else {
                continue;
            };
            let shape = field
                .holds
                .clone()
                .unwrap_or_else(|| field.type_name.clone());
            if !graph.ability_given(&shape, TAKES_MANY) {
                continue;
            }
            let declared = graph
                .types()
                .iter()
                .any(|candidate| candidate.kind == TypeKind::Struct && candidate.name == element);
            if !declared && !carried_to_a_file(workspace, &node.name, &field.name) {
                continue;
            }
            held.push(Store {
                owner: node.name.clone(),
                field: field.name.clone(),
                shape,
                record: element.to_owned(),
                key: field.key.clone(),
                wire: graph.ability_given(&node.name, "Serialize")
                    || graph.ability_given(&node.name, "Deserialize"),
                site: node.range,
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

/// Whether some method both touches this field and reaches a boundary
/// records outlive the run behind — the file system or the database —
/// in the same body.
///
/// A collection of values another crate declares is not obviously a
/// store of anything — until the program carries it across such a
/// boundary, which is what a program does with what it means to keep. A
/// collection built and dropped inside one body is never carried
/// anywhere and stays what it is: working memory.
fn carried_to_a_file(workspace: &Workspace<'_>, owner: &str, field: &str) -> bool {
    let key = format!("{owner}.{field}");
    workspace.facts().iter().any(|fact| {
        let touches = fact.reads.contains(&key)
            || fact.writes.contains(&key)
            || fact.builds.contains(&key)
            || builds_the_owner(fact, owner);
        touches
            && fact.effect_sites.iter().any(|site| {
                matches!(
                    site.effect,
                    Effect::FileSystem | Effect::DatabaseWrite | Effect::DatabaseRead
                )
            })
    })
}

/// Whether a body builds the type that declares the field.
///
/// A collection filled into a fresh value of its owner is that owner's
/// collection however the body spells the name: the constructor writes
/// `Self`, and the reading follows the declaration rather than the
/// spelling.
fn builds_the_owner(fact: &crate::semantic::behavior::FunctionFacts, owner: &str) -> bool {
    fact.constructs
        .iter()
        .any(|built| built == owner || built == "Self")
        && fact.function.starts_with(&format!("{owner}::"))
}
