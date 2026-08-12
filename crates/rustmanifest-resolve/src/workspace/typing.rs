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
use rustmanifest_schema::{ResolvedProperty, ResolvedType};

use super::reading::Reading;

/// Records the type of every expression of `parsed` that the resolver
/// settles, and what the compiler grants each type so recorded.
///
/// The grant travels with the type rather than with the position: whether
/// a value is copied when it is passed by value is fixed by its type, and
/// asking the compiler once per occurrence costs nothing beyond the answer
/// it already has to hand.
pub(super) fn settled(
    reading: Reading<'_, '_>,
    parsed: &ast::SourceFile,
    into: &mut Vec<ResolvedType>,
    granted: &mut Vec<ResolvedProperty>
) {
    for expression in parsed.syntax().descendants().filter_map(ast::Expr::cast) {
        if matches!(expression, ast::Expr::Literal(_)) {
            continue;
        }
        let Some(typed) = reading.semantics.type_of_expr(&expression) else {
            continue;
        };
        let typed = typed.original();
        let Some(rendered) = reading.render(&typed) else {
            continue;
        };
        granted.push(ResolvedProperty {
            copy:  typed.is_copy(reading.database),
            named: rendered.clone()
        });
        into.push(ResolvedType {
            at: reading.located(expression.syntax().text_range()),
            rendered
        });
    }
}
