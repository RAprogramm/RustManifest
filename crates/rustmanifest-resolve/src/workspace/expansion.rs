// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Reading what is written inside a macro argument.
//!
//! A field inside a macro argument is a token to a walk over the source
//! tree and an expression only after expansion — which is where the
//! positions a format string prints live. The expansion of a print
//! carries another macro inside it, so one level is not enough and the
//! walk goes two: far enough for the argument list, and bounded so a
//! macro defined in terms of itself cannot run the walk away.
//!
//! Every row read here is mapped back to the range the source wrote it at,
//! so a consumer never has to know a macro was involved.

use ra_ap_syntax::{AstNode, ast};
use rustmanifest_schema::Resolution;

use super::{
    reading::Reading,
    settling::{settled_call, settled_field, settled_free_call}
};

/// Records every field read and every method call written inside a macro
/// argument of `parsed`.
pub(super) fn settled(reading: Reading<'_, '_>, parsed: &ast::SourceFile, into: &mut Resolution) {
    for invocation in parsed
        .syntax()
        .descendants()
        .filter_map(ast::MacroCall::cast)
    {
        for tree in expanded(reading, &invocation) {
            for field in tree.descendants().filter_map(ast::FieldExpr::cast) {
                if let Some(mut settled) = settled_field(reading, &field) {
                    settled.at = written(reading, field.syntax());
                    into.fields.push(settled);
                }
            }
            for call in tree.descendants().filter_map(ast::MethodCallExpr::cast) {
                if let Some(mut settled) = settled_call(reading, &call) {
                    settled.at = written(reading, call.syntax());
                    into.calls.push(settled);
                }
            }
            for call in tree.descendants().filter_map(ast::CallExpr::cast) {
                if let Some(mut settled) = settled_free_call(reading, &call) {
                    settled.at = written(reading, call.syntax());
                    into.calls.push(settled);
                }
            }
        }
    }
}

/// Expands `invocation` and, one level deeper, every macro its expansion
/// carries.
fn expanded(
    reading: Reading<'_, '_>,
    invocation: &ast::MacroCall
) -> Vec<ra_ap_syntax::SyntaxNode> {
    let Some(expansion) = reading.semantics.expand_macro_call(invocation) else {
        return Vec::new();
    };
    let mut trees = vec![expansion.value.clone()];
    trees.extend(
        expansion
            .value
            .descendants()
            .filter_map(ast::MacroCall::cast)
            .filter_map(|inner| reading.semantics.expand_macro_call(&inner))
            .map(|deeper| deeper.value)
    );
    trees
}

/// Maps a node of an expansion back to the range the source wrote it at.
fn written(
    reading: Reading<'_, '_>,
    node: &ra_ap_syntax::SyntaxNode
) -> rustmanifest_schema::Location {
    reading.located(reading.semantics.original_range(node).range)
}
