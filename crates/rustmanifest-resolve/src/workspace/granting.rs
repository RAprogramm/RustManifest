// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Which traits each type of a file carries.
//!
//! A written `impl` and a derive are the same answer to the compiler and
//! two different readings to a text walker: one is a header to parse, the
//! other an attribute whose expansion has to be known. Asking the compiler
//! answers both at once, and answers for a trait a dependency declares.
//!
//! The types the fields hold are asked the same question, and that is
//! what puts a library type in reach: a reader deciding whether a field
//! can be printed or defaulted has the compiler's answer for `String` and
//! for `u32` exactly as it has it for a type this workspace declared.

use ra_ap_syntax::{AstNode, ast};
use rustmanifest_schema::ResolvedGrant;

use super::reading::Reading;

/// Records every trait the type of each field written in `parsed`
/// carries, whichever crate declares that type.
pub(super) fn settled_field_types(
    reading: Reading<'_, '_>,
    parsed: &ast::SourceFile,
    into: &mut Vec<ResolvedGrant>
) {
    for written in parsed.syntax().descendants().filter_map(ast::RecordField::cast) {
        let Some(field) = reading.semantics.to_def(&written) else {
            continue;
        };
        let held = field.ty(reading.database);
        let Some(named) = reading.render(&held) else {
            continue;
        };
        let at = reading.located(written.syntax().text_range());
        for block in ra_ap_hir::Impl::all_for_type(reading.database, held) {
            let Some(carried) = block.trait_(reading.database) else {
                continue;
            };
            into.push(ResolvedGrant {
                at: at.clone(),
                named: named.clone(),
                trait_name: carried.name(reading.database).as_str().to_owned()
            });
        }
    }
}

/// Records every trait the types declared in `parsed` carry.
pub(super) fn settled(
    reading: Reading<'_, '_>,
    parsed: &ast::SourceFile,
    into: &mut Vec<ResolvedGrant>
) {
    for written in parsed.syntax().descendants().filter_map(ast::Impl::cast) {
        let Some(block) = reading.semantics.to_def(&written) else {
            continue;
        };
        let Some(carried) = block.trait_(reading.database) else {
            continue;
        };
        let Some(named) = reading.render(&block.self_ty(reading.database)) else {
            continue;
        };
        into.push(ResolvedGrant {
            at: reading.located(written.syntax().text_range()),
            named,
            trait_name: carried.name(reading.database).as_str().to_owned()
        });
    }
}
