// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The stream a written-to destination names, read from the handle it is
//! built from, from the type its binding was declared with or initialized
//! from, or from the fact that the caller handed it in — together with
//! where the stream's lock was taken relative to the site.
//!
//! Reading an expression on its own terms is the sibling module's job;
//! what a name stands for is answered here, against the binding tables.

mod handle;

use self::handle::{destination_root, guard_of, named_stream, standard_handle, stream_type};
use super::{
    FactCollector,
    catalog::{STREAM_GUARD_SUFFIX, STREAM_LOCK_METHOD}
};
use crate::semantic::behavior::output::{Acquisition, Buffering, Stream};

/// What a write site's destination expression resolves to: the stream the
/// bytes reach, where that stream's lock was taken, and how the
/// destination buffers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Destination {
    /// Where the bytes go.
    pub(super) stream:      Stream,
    /// Where the stream's lock was taken relative to the site.
    pub(super) acquisition: Acquisition,
    /// How the destination buffers what the site writes.
    pub(super) buffering:   Buffering
}

impl Destination {
    /// A destination nothing could be proven about.
    const UNKNOWN: Self = Self {
        stream:      Stream::Unknown,
        acquisition: Acquisition::Unknown,
        buffering:   Buffering::Unknown
    };

    /// A destination the caller handed in, so it points wherever the
    /// caller pointed it and no lock of this process's own is claimed.
    const CALLER_SUPPLIED: Self = Self {
        stream:      Stream::CallerSupplied,
        acquisition: Acquisition::Unknown,
        buffering:   Buffering::Unknown
    };
}

impl FactCollector {
    /// The stream a written-to destination names, and where its lock was
    /// taken.
    ///
    /// Three questions, asked in order and none guessed: whether the
    /// destination is built from one of the process's own handles at the
    /// site, whether it names a binding whose recorded type is one of
    /// those handles, and whether it arrives from outside this function —
    /// a writer this function is handed is the caller's to point wherever
    /// it likes. Anything else is recorded as unidentified rather than
    /// assumed.
    ///
    /// The lock reading follows from the same answer. A site that builds
    /// the handle takes the lock itself, whether it locks explicitly or
    /// leaves the write method to lock per call. A site writing through a
    /// binding takes it only when that binding holds the handle rather
    /// than a guard: a guard was locked once, where it was bound.
    pub(super) fn write_destination(&self, destination: &syn::Expr) -> Destination {
        if let Some(stream) = self.handle_stream(destination) {
            return Destination {
                stream,
                acquisition: Acquisition::AtSite,
                buffering: Buffering::StreamOwn
            };
        }
        let syn::Expr::Path(path) = destination_root(destination) else {
            return Destination::UNKNOWN;
        };
        let Some(name) = path.path.get_ident().map(ToString::to_string) else {
            return Destination::UNKNOWN;
        };
        if let Some(stream) = self.buffered_streams.get(&name) {
            return Destination {
                stream:      *stream,
                acquisition: Acquisition::Held,
                buffering:   Buffering::Block
            };
        }
        if let Some(type_name) = self.param_types.get(&name)
            && let Some(stream) = named_stream(type_name)
        {
            return Destination {
                stream,
                acquisition: if type_name.ends_with(STREAM_GUARD_SUFFIX) {
                    Acquisition::Held
                } else {
                    Acquisition::AtSite
                },
                buffering: Buffering::StreamOwn
            };
        }
        if self.facts.params.iter().any(|param| param.name == name) {
            return Destination::CALLER_SUPPLIED;
        }
        Destination::UNKNOWN
    }

    /// The process stream a written buffering wrapper is bound over:
    /// `BufWriter::new(inner)` or `BufWriter::with_capacity(_, inner)`
    /// where `inner` resolves to one of the process's own streams. The
    /// stream identity carries through the wrapper; the buffering is
    /// the wrapper's own, which is the point of writing one.
    pub(super) fn buffered_stream_of(&self, expression: &syn::Expr) -> Option<Stream> {
        let syn::Expr::Call(call) = expression else {
            return None;
        };
        let syn::Expr::Path(path) = call.func.as_ref() else {
            return None;
        };
        let segments = self.expanded_segments(&path.path);
        let [.., wrapper, method] = segments.as_slice() else {
            return None;
        };
        if wrapper != "BufWriter" {
            return None;
        }
        let inner = match method.as_str() {
            "new" => call.args.first(),
            "with_capacity" => call.args.iter().nth(1),
            _ => None
        }?;
        match self.write_destination(inner) {
            Destination {
                stream: stream @ (Stream::Stdout | Stream::Stderr),
                ..
            } => Some(stream),
            _ => None
        }
    }

    /// The library type an initializer expression produces, where that
    /// type is one of the process's own stream handles or its guard.
    ///
    /// Only the locking call is followed, and only onto a handle. Every
    /// other call over a handle produces something this tier cannot name
    /// — a wrapper, an adapter, a value of the caller's own — and a
    /// binding holding it is left unresolved rather than credited to the
    /// stream.
    pub(super) fn handle_type(&self, expression: &syn::Expr) -> Option<&'static str> {
        match expression {
            syn::Expr::Reference(reference) => self.handle_type(&reference.expr),
            syn::Expr::Paren(inner) => self.handle_type(&inner.expr),
            syn::Expr::Group(inner) => self.handle_type(&inner.expr),
            syn::Expr::MethodCall(call) if call.method == STREAM_LOCK_METHOD => {
                guard_of(self.handle_type(&call.receiver)?)
            }
            syn::Expr::Call(call) => self.constructed_handle(call).map(|(handle, _)| handle),
            syn::Expr::Path(path) => {
                let name = path.path.get_ident()?.to_string();
                stream_type(self.param_types.get(&name)?)
            }
            _ => None
        }
    }

    /// The process stream a destination expression is built from: the
    /// standard-library handle constructors, seen through the borrows and
    /// the locking calls that hand out a guard over the same stream.
    ///
    /// A buffering wrapper is deliberately not seen through. It is a
    /// different destination: bytes handed to it reach the process stream
    /// when the buffer is flushed and not when they are written, which is
    /// the whole point of putting one there. Claiming otherwise would name
    /// the canonical form as a write to a line-buffered handle.
    fn handle_stream(&self, expression: &syn::Expr) -> Option<Stream> {
        match expression {
            syn::Expr::Reference(reference) => self.handle_stream(&reference.expr),
            syn::Expr::Paren(inner) => self.handle_stream(&inner.expr),
            syn::Expr::Group(inner) => self.handle_stream(&inner.expr),
            syn::Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Deref(_)) => {
                self.handle_stream(&unary.expr)
            }
            syn::Expr::MethodCall(call) => self.handle_stream(&call.receiver),
            syn::Expr::Call(call) => self.constructed_handle(call).map(|(_, stream)| stream),
            _ => None
        }
    }

    /// The standard stream handle a call constructs, with the stream it
    /// leads to, decided on the path the call expands to through this
    /// file's imports rather than on the name it is spelled with.
    fn constructed_handle(&self, call: &syn::ExprCall) -> Option<(&'static str, Stream)> {
        let syn::Expr::Path(path) = call.func.as_ref() else {
            return None;
        };
        standard_handle(&self.expanded_segments(&path.path))
    }
}
