// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a resolved name stands for: the standard stream a constructor
//! path leads to, the types that stream's handle and guard are written
//! as, and the expression a destination is ultimately reached from.
//!
//! Every answer here is a lookup against the catalogue of standard-library
//! paths and type names. Deciding which path a call actually expands to is
//! the caller's job, since only the caller holds the file's imports.

use super::super::catalog::{STREAM_HANDLE_PATHS, STREAM_TYPES};
use crate::semantic::behavior::output::Stream;

/// The stream a fully expanded constructor path leads to, with the handle
/// type it returns.
pub(super) fn standard_handle(segments: &[String]) -> Option<(&'static str, Stream)> {
    STREAM_HANDLE_PATHS
        .iter()
        .find(|(path, _, _, _)| path.len() == segments.len() && matched(path, segments))
        .map(|(_, stream, handle, _)| (*handle, *stream))
}

/// Whether an expanded path is the catalogued one, segment by segment.
fn matched(path: &[&str], segments: &[String]) -> bool {
    path.iter()
        .zip(segments)
        .all(|(named, segment)| *named == segment.as_str())
}

/// The guard type the locking call over `handle` hands out.
pub(super) fn guard_of(handle: &str) -> Option<&'static str> {
    STREAM_HANDLE_PATHS
        .iter()
        .find(|(_, _, named, _)| *named == handle)
        .map(|(_, _, _, guard)| *guard)
}

/// The static spelling of a recorded type name, where that name is one of
/// the process's own stream handles or its guard.
pub(super) fn stream_type(recorded: &str) -> Option<&'static str> {
    STREAM_HANDLE_PATHS
        .iter()
        .find_map(|(_, _, handle, guard)| match recorded {
            named if named == *handle => Some(*handle),
            named if named == *guard => Some(*guard),
            _ => None
        })
}

/// The process stream the type at a path stands for, and whether a
/// value of it already holds the lock.
pub(super) fn named_stream(declared: &str) -> Option<(Stream, bool)> {
    STREAM_TYPES
        .iter()
        .find(|(named, _, _)| *named == declared)
        .map(|(_, stream, held)| (*stream, *held))
}

/// The expression a write destination is ultimately reached from,
/// borrows, parentheses and the method chain over it stripped.
pub(super) fn destination_root(expression: &syn::Expr) -> &syn::Expr {
    match expression {
        syn::Expr::Reference(reference) => destination_root(&reference.expr),
        syn::Expr::Paren(inner) => destination_root(&inner.expr),
        syn::Expr::Group(inner) => destination_root(&inner.expr),
        syn::Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Deref(_)) => {
            destination_root(&unary.expr)
        }
        syn::Expr::MethodCall(call) => destination_root(&call.receiver),
        _ => expression
    }
}
