// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The type of every expression written in one file.
//!
//! This is the widening the whole overlay turns on. Asking the type of a
//! call and of a field access answers two spellings; a reader standing on
//! any other expression — a binding read, an index, a match, an await, a
//! turbofish, a cast, a block's value — was left to guess from how the
//! expression was written. Every such guess is a table of names somewhere
//! downstream. Asking the question for every expression removes the tables
//! rather than improving them.
//!
//! A literal is skipped: its type is not a question, and recording one row
//! per literal doubles the overlay to say what the text already says.

use ra_ap_syntax::{AstNode, ast};
use rustmanifest_schema::ResolvedType;

use super::reading::Reading;

/// Records the type of every expression of `parsed` that the resolver
/// settles.
pub(super) fn settled(
    reading: Reading<'_, '_>,
    parsed: &ast::SourceFile,
    into: &mut Vec<ResolvedType>
) {
    for expression in parsed.syntax().descendants().filter_map(ast::Expr::cast) {
        if let Some(settled) = settled_expression(reading, &expression) {
            into.push(settled);
        }
    }
}

/// Reads one expression, answering `None` where the resolver settled
/// nothing or the expression states its own type.
fn settled_expression(reading: Reading<'_, '_>, expression: &ast::Expr) -> Option<ResolvedType> {
    if matches!(expression, ast::Expr::Literal(_)) {
        return None;
    }
    let typed = reading.semantics.type_of_expr(expression)?.original();
    Some(ResolvedType {
        at:       reading.located(expression.syntax().text_range()),
        rendered: reading.render(&typed)?
    })
}
