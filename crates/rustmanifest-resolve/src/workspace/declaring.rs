// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Every function a file declares, named the way its crate names it.
//!
//! A body is recorded by the place it is written at and by the full path
//! that reaches it. That pairing is what lets a reader hold facts about a
//! body and still know which body they are about: two types with a `new`
//! share the written name and never the path.

use ra_ap_syntax::{AstNode, ast};
use rustmanifest_schema::ResolvedFunction;

use super::reading::Reading;

/// Records every function declared in `parsed`.
pub(super) fn settled(
    reading: Reading<'_, '_>,
    parsed: &ast::SourceFile,
    into: &mut Vec<ResolvedFunction>
) {
    for declared in parsed.syntax().descendants().filter_map(ast::Fn::cast) {
        let Some(function) = reading.semantics.to_def(&declared) else {
            continue;
        };
        let Some(names) = super::naming::canonical_function(reading.database, function) else {
            continue;
        };
        into.push(ResolvedFunction {
            at: reading.located(declared.syntax().text_range()),
            names
        });
    }
}
