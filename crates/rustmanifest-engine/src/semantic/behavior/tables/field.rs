// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Field typing tables: every named and positional field of every
//! struct in the workspace, keyed `Type.field`, read three ways — the
//! field's own type, the element type of a collection field, and the
//! inner type of a single-generic wrapper field.

use crate::{
    semantic::behavior::{
        scoped::Scoped,
        signature::{element_type, type_last_segment, wrapped_type}
    },
    source::Source
};

/// Maps every named field of a uniquely named struct — keyed
/// `Type.field` — to its type's last segment, every collection-typed
/// field to its element type, and every single-generic wrapper field
/// to its wrapped inner type.
pub(super) fn field_tables(
    sources: &[Source]
) -> (Scoped<String>, Scoped<String>, Scoped<String>) {
    let mut plain = Scoped::default();
    let mut elems = Scoped::default();
    let mut wraps = Scoped::default();
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
                if let Some(element) = element_type(&field.ty) {
                    elems.note(key.clone(), index, element);
                }
                if let Some(inner) = wrapped_type(&field.ty) {
                    wraps.note(key, index, inner);
                }
            }
        }
    }
    (plain, elems, wraps)
}
