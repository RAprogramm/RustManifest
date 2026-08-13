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
//!
//! Every name a pattern binds is asked for too. A closure's input and a
//! loop's variable are written without a type and read everywhere, and
//! what they stand for is decided by the function that hands them over —
//! which is why answering it from the receiver's spelling took a list of
//! adapter names per question asked, and answering it from the compiler
//! takes one call.

use ra_ap_syntax::{AstNode, ast};
use rustmanifest_schema::{ResolvedLayout, ResolvedProperty, ResolvedType};

use super::reading::Reading;

/// Records the type of every expression of `parsed` that the resolver
/// settles, what the compiler grants each type so recorded, and what the
/// compiler says a value of it weighs.
///
/// The grant travels with the type rather than with the position: whether
/// a value is copied when it is passed by value is fixed by its type, and
/// asking the compiler once per occurrence costs nothing beyond the answer
/// it already has to hand.
pub(super) fn settled(
    reading: Reading<'_, '_>,
    parsed: &ast::SourceFile,
    into: &mut Vec<ResolvedType>,
    granted: &mut Vec<ResolvedProperty>,
    weighs: &mut Vec<ResolvedLayout>
) {
    for expression in parsed.syntax().descendants().filter_map(ast::Expr::cast) {
        if matches!(expression, ast::Expr::Literal(_)) {
            continue;
        }
        let Some(typed) = reading.semantics.type_of_expr(&expression) else {
            continue;
        };
        record(
            reading,
            &typed.original(),
            expression.syntax().text_range(),
            into,
            granted,
            weighs
        );
    }
    for bound in parsed.syntax().descendants().filter_map(ast::IdentPat::cast) {
        let Some(typed) = reading.semantics.type_of_binding_in_pat(&bound) else {
            continue;
        };
        let at = ast::HasName::name(&bound).map_or_else(
            || bound.syntax().text_range(),
            |written| written.syntax().text_range()
        );
        record(reading, &typed, at, into, granted, weighs);
    }
}

/// Records one settled type at one position, with what the compiler
/// grants it and what it says a value of it weighs.
fn record(
    reading: Reading<'_, '_>,
    typed: &ra_ap_hir::Type<'_>,
    at: ra_ap_syntax::TextRange,
    into: &mut Vec<ResolvedType>,
    granted: &mut Vec<ResolvedProperty>,
    weighs: &mut Vec<ResolvedLayout>
) {
    let Some(rendered) = reading.render(typed) else {
        return;
    };
    granted.push(ResolvedProperty {
        copy:  typed.is_copy(reading.database),
        named: rendered.clone()
    });
    if let Ok(held) = typed.layout(reading.database) {
        weighs.push(ResolvedLayout {
            named: rendered.clone(),
            size:  held.size(),
            align: held.align()
        });
    }
    into.push(ResolvedType {
        at: reading.located(at),
        rendered,
        declared: typed
            .strip_references()
            .as_adt()
            .and_then(|held| super::naming::canonical_type(reading.database, held))
    });
}
