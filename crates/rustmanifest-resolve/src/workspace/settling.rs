// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Reading one call and one field access.

use ra_ap_syntax::{AstNode, ast};
use rustmanifest_schema::{ResolvedCall, ResolvedField};

use super::reading::Reading;

/// Reads one field access, answering `None` where the resolver settled
/// nothing.
///
/// Both types are asked for: the one holding the field, which is what
/// the engine names a field by, and the one the field itself has, which
/// is what a reading standing on it needs. Either coming back unknown
/// leaves the position out, because a half-answer here would be read as
/// a whole one.
pub(super) fn settled_field(
    reading: Reading<'_, '_>,
    field: &ast::FieldExpr
) -> Option<ResolvedField> {
    let base = field.expr()?;
    let owner_type = reading.semantics.type_of_expr(&base)?.original();
    let produced_type = reading
        .semantics
        .type_of_expr(&ast::Expr::FieldExpr(field.clone()))?
        .original();
    Some(ResolvedField {
        at:       reading.located(field.syntax().text_range()),
        owner:    reading.render(&owner_type)?,
        produced: reading.render(&produced_type)?
    })
}

/// Reads one free call — a call written as a path rather than on a
/// receiver — answering `None` where the resolver settled nothing.
///
/// A method call and a free call are two spellings of the same event, and
/// a reading that settles only the first leaves every `persist(entry)` to
/// be guessed from its written name.
pub(super) fn settled_free_call(
    reading: Reading<'_, '_>,
    call: &ast::CallExpr
) -> Option<ResolvedCall> {
    let ast::Expr::PathExpr(written) = call.expr()? else {
        return None;
    };
    let ra_ap_hir::PathResolution::Def(ra_ap_hir::ModuleDef::Function(function)) =
        reading.semantics.resolve_path(&written.path()?)?
    else {
        return None;
    };
    let returns = function.ret_type(reading.database);
    Some(ResolvedCall {
        at:       reading.located(call.syntax().text_range()),
        callee:   function.name(reading.database).as_str().to_owned(),
        returns:  reading.render(&returns)?,
        receiver: None,
        reaches:  super::naming::canonical_function(reading.database, function)
    })
}

/// Reads one method call, answering `None` where the resolver settled
/// nothing.
pub(super) fn settled_call(
    reading: Reading<'_, '_>,
    call: &ast::MethodCallExpr
) -> Option<ResolvedCall> {
    let function = reading.semantics.resolve_method_call(call)?;
    let returns = function.ret_type(reading.database);
    let reaches = super::naming::canonical_function(reading.database, function);
    Some(ResolvedCall {
        at: reading.located(call.syntax().text_range()),
        callee: function.name(reading.database).as_str().to_owned(),
        returns: reading.render(&returns)?,
        receiver: call
            .receiver()
            .and_then(|expression| reading.semantics.type_of_expr(&expression))
            .and_then(|typed| reading.render(&typed.original())),
        reaches
    })
}
