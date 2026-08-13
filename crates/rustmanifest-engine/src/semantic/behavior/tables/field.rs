// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Field typing tables: every named and positional field of every
//! struct in the workspace, keyed `Type.field`, read four ways — the
//! field's own type, the type a lookup into it hands out (for a map that
//! is its value type, for a sequence its element), the key and value
//! types of a map, and the inner type of a single-generic wrapper.

use crate::{
    semantic::behavior::{
        scoped::Scoped,
        signature::{pair_types, type_last_segment, wrapped_type}
    },
    source::Source
};

/// The four readings of a field the tables carry.
pub(super) struct FieldTables {
    /// The field's own type, by last segment.
    pub(super) plain: Scoped<String>,
    /// The inner type of a single-generic wrapper field.
    pub(super) wraps: Scoped<String>,
    /// The key and value types of a map field.
    pub(super) pairs: Scoped<(String, String)>
}

/// Maps every named field of a uniquely named struct — keyed
/// `Type.field` — to its type's last segment, every collection-typed
/// field to its element type, and every single-generic wrapper field
/// to its wrapped inner type.
pub(super) fn field_tables(sources: &[Source]) -> FieldTables {
    let mut plain = Scoped::default();
    let mut wraps = Scoped::default();
    let mut pairs = Scoped::default();
    for (index, source) in sources.iter().enumerate() {
        let Some(file) = source.tree() else {
            continue;
        };
        let mut items = Vec::new();
        crate::ast::flattened(&file.items, &mut items);
        for item in items {
            let syn::Item::Struct(definition) = item else {
                continue;
            };
            for (position, field) in definition.fields.iter().enumerate() {
                let member = field
                    .ident
                    .as_ref()
                    .map_or_else(|| position.to_string(), ToString::to_string);
                let key = format!("{}.{member}", definition.ident);
                if let Some(type_name) = type_last_segment(&field.ty) {
                    plain.note(key.clone(), index, type_name);
                }
                if let Some(pair) = pair_types(&field.ty) {
                    pairs.note(key.clone(), index, pair);
                }
                if let Some(inner) = wrapped_type(&field.ty) {
                    wraps.note(key, index, inner);
                }
            }
        }
    }
    FieldTables {
        plain,
        wraps,
        pairs
    }
}
