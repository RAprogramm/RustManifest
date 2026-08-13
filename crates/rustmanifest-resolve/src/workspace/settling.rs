// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Reading one call and one field access.

use ra_ap_syntax::{AstNode, ast};
use rustmanifest_schema::{ResolvedCall, ResolvedField, ResolvedMode};

use super::reading::Reading;

/// The full path of a receiver's own type, where it has one an item
/// declares.
///
/// A borrowed handle, a boxed one and a plain one are the same type to
/// the question being asked, so the rendering is peeled to the type
/// itself before its declaration is named.
fn receiver_path(reading: Reading<'_, '_>, typed: &ra_ap_hir::Type<'_>) -> Option<String> {
    let held = typed.strip_references();
    let declared = held.as_adt()?;
    super::naming::canonical_type(reading.database, declared)
}

/// How the function a call reaches takes its receiver, where it has one.
///
/// A free function has none, and the row stays absent rather than
/// claiming a mode: absent means the question does not arise, and a
/// reading that turned it into "takes it by value" would credit every
/// path-written call with a move.
fn receiver_mode(
    database: &dyn ra_ap_hir::db::HirDatabase,
    function: ra_ap_hir::Function
) -> Option<ResolvedMode> {
    Some(match function.self_param(database)?.access(database) {
        ra_ap_hir::Access::Shared => ResolvedMode::Borrowed,
        ra_ap_hir::Access::Exclusive => ResolvedMode::BorrowedMut,
        ra_ap_hir::Access::Owned => ResolvedMode::Owned
    })
}

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
        receiver_path: None,
        reaches:  super::naming::canonical_function(reading.database, function),
        takes:    receiver_mode(reading.database, function)
    })
}

/// Reads one method call, answering `None` where the resolver settled
/// nothing.
///
/// What the call hands back is asked of the call expression itself and
/// not of the declaration: a method declared to hand back `Self` renders
/// as those four letters, which name the type only to a reader standing
/// inside that impl. The expression renders as the type it actually has.
pub(super) fn settled_call(
    reading: Reading<'_, '_>,
    call: &ast::MethodCallExpr
) -> Option<ResolvedCall> {
    let function = reading.semantics.resolve_method_call(call)?;
    let reaches = super::naming::canonical_function(reading.database, function);
    let returns = reading
        .semantics
        .type_of_expr(&ast::Expr::MethodCallExpr(call.clone()))
        .map(|typed| typed.original())
        .and_then(|typed| reading.render(&typed))
        .or_else(|| reading.render(&function.ret_type(reading.database)))?;
    let received = call
        .receiver()
        .and_then(|expression| reading.semantics.type_of_expr(&expression))
        .map(|typed| typed.original());
    Some(ResolvedCall {
        at: reading.located(call.syntax().text_range()),
        callee: function.name(reading.database).as_str().to_owned(),
        returns,
        receiver_path: received
            .as_ref()
            .and_then(|typed| receiver_path(reading, typed)),
        receiver: received.as_ref().and_then(|typed| reading.render(typed)),
        reaches,
        takes: receiver_mode(reading.database, function)
    })
}
