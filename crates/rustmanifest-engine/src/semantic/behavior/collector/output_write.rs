// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Recording one write to an output stream, in body order: the print
//! macros whose stream their own name fixes, the writer macros that take
//! their destination as an argument, and the byte-slice write method.

use super::{FactCollector, destination::Destination};
use crate::semantic::behavior::output::{
    self, Acquisition, Conduit, Newline, OpaqueReason, OutputWrite, Payload, Stream
};

impl FactCollector {
    /// Records one write to an output stream, in body order.
    ///
    /// The payload is whatever the construct proves and no more; the
    /// byte range of the construct is kept so a claim can point at what
    /// produced it, and the machinery it travelled through is kept beside
    /// it so a form that costs only one of them is never claimed of the
    /// other.
    fn push_output(
        &mut self,
        span: std::ops::Range<usize>,
        destination: Destination,
        payload: Payload,
        newline: Newline,
        conduit: Conduit
    ) {
        self.facts.output_writes.push(OutputWrite {
            stream: destination.stream,
            payload,
            newline,
            site: rustmanifest_schema::Range {
                start: u32::try_from(span.start).unwrap_or(u32::MAX),
                end:   u32::try_from(span.end).unwrap_or(u32::MAX)
            },
            guard: self.site_guard(),
            multiplicity: output::Multiplicity::Once,
            conduit,
            acquisition: destination.acquisition
        });
    }

    /// Records one write by a print-family macro, whose stream is fixed
    /// by the macro's own name.
    ///
    /// The macro formats its arguments and takes the stream's lock for
    /// the one call, so both readings are fixed by the construct itself.
    pub(super) fn record_output(
        &mut self,
        invocation: &syn::Macro,
        stream: Stream,
        newline: Newline,
        debug_macro: bool
    ) {
        use syn::spanned::Spanned;
        self.push_output(
            invocation.span().byte_range(),
            Destination {
                stream,
                acquisition: Acquisition::AtSite
            },
            output::macro_payload(invocation, debug_macro),
            newline,
            Conduit::Formatting
        );
    }

    /// Records one write by a macro that takes its destination as the
    /// first argument. The destination decides the stream, the remaining
    /// arguments carry the payload, and an invocation whose arguments do
    /// not parse is still recorded — with both answers stated as
    /// unproven rather than dropped.
    pub(super) fn record_writer_output(&mut self, invocation: &syn::Macro, newline: Newline) {
        use syn::spanned::Spanned;
        let span = invocation.span().byte_range();
        let unreadable = Payload::Opaque(OpaqueReason::Unparsed);
        let unidentified = Destination {
            stream:      Stream::Unknown,
            acquisition: Acquisition::Unknown
        };
        let Ok(arguments) = invocation.parse_body_with(
            syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated
        ) else {
            self.push_output(span, unidentified, unreadable, newline, Conduit::Formatting);
            return;
        };
        let written: Vec<&syn::Expr> = arguments.iter().collect();
        let Some((destination, rest)) = written.split_first() else {
            self.push_output(span, unidentified, unreadable, newline, Conduit::Formatting);
            return;
        };
        let resolved = self.write_destination(destination);
        let payload = output::argument_payload(rest);
        self.push_output(span, resolved, payload, newline, Conduit::Formatting);
    }

    /// Records one write by the byte-slice write method of the write
    /// trait: the receiver decides the stream, and the bytes are proven
    /// only where they are written as a literal.
    pub(super) fn record_byte_output(&mut self, call: &syn::ExprMethodCall) {
        use syn::spanned::Spanned;
        let destination = self.write_destination(&call.receiver);
        let payload = call
            .args
            .first()
            .map_or(Payload::Empty, output::literal_bytes_payload);
        self.push_output(
            call.span().byte_range(),
            destination,
            payload,
            Newline::None,
            Conduit::Bytes
        );
    }
}
