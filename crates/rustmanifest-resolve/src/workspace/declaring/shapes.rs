// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Every type a file declares, with the type of each field as the
//! compiler reads it.

use ra_ap_syntax::{AstNode, ast, ast::HasName as _};
use rustmanifest_schema::{ResolvedFieldDecl, ResolvedTypeDecl};

use crate::workspace::reading::Reading;

/// Records every structure declared in `parsed`.
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
