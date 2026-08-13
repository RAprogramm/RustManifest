// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Every function a file declares, named the way its crate names it.
//!
//! A body is recorded by the place its name is written at — the name, not
//! the whole declaration, because that is the position a reader holding
//! facts about the body already has — and by the full path that reaches
//! it. That pairing is what lets a reader hold facts about a
//! body and still know which body they are about: two types with a `new`
//! share the written name and never the path.

mod shapes;

pub(super) use self::shapes::settled as shapes_settled;

use ra_ap_syntax::{AstNode, ast, ast::HasName as _};
use rustmanifest_schema::{ResolvedFunction, ResolvedMode, ResolvedParam, ResolvedSignature};

use super::reading::Reading;

/// What one function takes and hands back, as the compiler reads it.
fn signature_of(
    reading: Reading<'_, '_>,
    function: ra_ap_hir::Function,
    at: rustmanifest_schema::Location
) -> Option<ResolvedSignature> {
    let database = reading.database;
    let mut params = Vec::new();
    if let Some(receiver) = function.self_param(database) {
        params.push(ResolvedParam {
            name:  "self".to_owned(),
            named: reading.render(&receiver.ty(database)).unwrap_or_default(),
            mode:  match receiver.access(database) {
                ra_ap_hir::Access::Shared => ResolvedMode::Borrowed,
                ra_ap_hir::Access::Exclusive => ResolvedMode::BorrowedMut,
                ra_ap_hir::Access::Owned => ResolvedMode::Owned
            }
        });
    }
    for held in function.params_without_self(database) {
        let named = reading.render(held.ty())?;
        params.push(ResolvedParam {
            name: held
                .name(database)
                .map(|written| written.as_str().to_owned())
                .unwrap_or_default(),
            mode: mode_of(&named),
            named
        });
    }
    Some(ResolvedSignature {
        at,
        params,
        returns: reading.render(&function.ret_type(database))?
    })
}

/// How a parameter is taken, read off the type the compiler renders.
fn mode_of(named: &str) -> ResolvedMode {
    if named.starts_with("&mut ") {
        ResolvedMode::BorrowedMut
    } else if named.starts_with('&') {
        ResolvedMode::Borrowed
    } else {
        ResolvedMode::Owned
    }
}

/// Records every function declared in `parsed`.
pub(super) fn settled(
    reading: Reading<'_, '_>,
    parsed: &ast::SourceFile,
    into: &mut Vec<ResolvedFunction>,
    signatures: &mut Vec<ResolvedSignature>
) {
    for declared in parsed.syntax().descendants().filter_map(ast::Fn::cast) {
        let Some(function) = reading.semantics.to_def(&declared) else {
            continue;
        };
        let Some(names) = super::naming::canonical_function(reading.database, function) else {
            continue;
        };
        let Some(written) = declared.name() else {
            continue;
        };
        let at = reading.located(written.syntax().text_range());
        if let Some(signature) = signature_of(reading, function, at.clone()) {
            signatures.push(signature);
        }
        into.push(ResolvedFunction {
            at,
            names
        });
    }
}
