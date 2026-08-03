// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The stream a written-to destination names, read from the handle it is
//! built from, from the type its binding was declared with, or from the
//! fact that the caller handed it in.

use super::{
    FactCollector,
    catalog::{STREAM_HANDLES, STREAM_TYPES}
};
use crate::semantic::behavior::output::Stream;

impl FactCollector {
    /// The stream a written-to destination names.
    ///
    /// Two questions, asked in order and neither guessed: whether the
    /// destination is built from one of the process's own handles, and
    /// whether it arrives from outside this function — a writer this
    /// function is handed is the caller's to point wherever it likes.
    /// Anything else is recorded as unidentified rather than assumed.
    pub(super) fn write_destination(&self, destination: &syn::Expr) -> Stream {
        if let Some(stream) = handle_stream(destination) {
            return stream;
        }
        let syn::Expr::Path(path) = destination_root(destination) else {
            return Stream::Unknown;
        };
        let Some(name) = path.path.get_ident().map(ToString::to_string) else {
            return Stream::Unknown;
        };
        if let Some(stream) = self
            .param_types
            .get(&name)
            .and_then(|type_name| named_stream(type_name))
        {
            return stream;
        }
        if self.facts.params.iter().any(|param| param.name == name) {
            return Stream::CallerSupplied;
        }
        Stream::Unknown
    }
}

/// The process stream a destination expression is built from: the
/// handle constructors, seen through the borrows and the locking calls
/// that hand out a guard over the same stream.
///
/// A buffering wrapper is deliberately not seen through. It is a
/// different destination: bytes handed to it reach the process stream
/// when the buffer is flushed and not when they are written, which is
/// the whole point of putting one there. Claiming otherwise would name
/// the canonical form as a write to a line-buffered handle.
fn handle_stream(expression: &syn::Expr) -> Option<Stream> {
    match expression {
        syn::Expr::Reference(reference) => handle_stream(&reference.expr),
        syn::Expr::Paren(inner) => handle_stream(&inner.expr),
        syn::Expr::Group(inner) => handle_stream(&inner.expr),
        syn::Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Deref(_)) => {
            handle_stream(&unary.expr)
        }
        syn::Expr::MethodCall(call) => handle_stream(&call.receiver),
        syn::Expr::Call(call) => {
            let syn::Expr::Path(path) = call.func.as_ref() else {
                return None;
            };
            let name = path.path.segments.last()?.ident.to_string();
            STREAM_HANDLES
                .iter()
                .find(|(handle, _)| *handle == name.as_str())
                .map(|(_, stream)| *stream)
        }
        _ => None
    }
}

/// The process stream a written type name stands for.
fn named_stream(type_name: &str) -> Option<Stream> {
    STREAM_TYPES
        .iter()
        .find(|(named, _)| *named == type_name)
        .map(|(_, stream)| *stream)
}

/// The expression a write destination is ultimately reached from,
/// borrows, parentheses and the method chain over it stripped.
fn destination_root(expression: &syn::Expr) -> &syn::Expr {
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
