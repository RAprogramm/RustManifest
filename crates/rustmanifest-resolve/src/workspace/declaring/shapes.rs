// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Every type a file declares, with the type of each field as the
//! compiler reads it.

use ra_ap_syntax::{AstNode, ast, ast::HasName as _};
use rustmanifest_schema::{ResolvedFieldDecl, ResolvedTypeDecl};

use crate::workspace::reading::Reading;

/// Records every structure and enumeration declared in `parsed`.
///
/// An enumeration's payloads are named the way a reader names them —
/// `Variant.field`, or the variant alone where the payload is unnamed —
/// so one row shape answers for both kinds of declaration.
pub(in crate::workspace) fn settled(
    reading: Reading<'_, '_>,
    parsed: &ast::SourceFile,
    into: &mut Vec<ResolvedTypeDecl>
) {
    for declared in parsed.syntax().descendants().filter_map(ast::Struct::cast) {
        let Some(definition) = reading.semantics.to_def(&declared) else {
            continue;
        };
        let Some(written) = declared.name() else {
            continue;
        };
        into.push(ResolvedTypeDecl {
            at:     reading.located(written.syntax().text_range()),
            named:  definition.name(reading.database).as_str().to_owned(),
            fields: definition
                .fields(reading.database)
                .into_iter()
                .filter_map(|field| {
                    Some(ResolvedFieldDecl {
                        name:  field.name(reading.database).as_str().to_owned(),
                        named: reading.render(&field.ty(reading.database))?
                    })
                })
                .collect()
        });
    }
}

/// Records every enumeration declared in `parsed`.
pub(in crate::workspace) fn settled_enums(
    reading: Reading<'_, '_>,
    parsed: &ast::SourceFile,
    into: &mut Vec<ResolvedTypeDecl>
) {
    for declared in parsed.syntax().descendants().filter_map(ast::Enum::cast) {
        let Some(definition) = reading.semantics.to_def(&declared) else {
            continue;
        };
        let Some(written) = declared.name() else {
            continue;
        };
        let mut fields = Vec::new();
        for variant in definition.variants(reading.database) {
            let held = variant.name(reading.database).as_str().to_owned();
            for field in variant.fields(reading.database) {
                let named = field.name(reading.database).as_str().to_owned();
                let Some(rendered) = reading.render(&field.ty(reading.database)) else {
                    continue;
                };
                let spelled = if named.chars().all(|letter| letter.is_ascii_digit()) {
                    held.clone()
                } else {
                    format!("{held}.{named}")
                };
                fields.push(ResolvedFieldDecl {
                    name:  spelled,
                    named: rendered
                });
            }
        }
        into.push(ResolvedTypeDecl {
            at: reading.located(written.syntax().text_range()),
            named: definition.name(reading.database).as_str().to_owned(),
            fields
        });
    }
}
