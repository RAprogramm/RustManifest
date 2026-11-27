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
            buffering: destination.buffering,
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
        let parsed = invocation
            .parse_body_with(
                syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated
            )
            .ok();
        let arguments: Vec<&syn::Expr> = parsed
            .as_ref()
            .map_or_default(|written| written.iter().skip(1).collect());
        let payload = self.typed_holes(output::macro_payload(invocation, debug_macro), &arguments);
        self.push_output(
            invocation.span().byte_range(),
            Destination {
                stream,
                acquisition: Acquisition::AtSite,
                buffering: output::Buffering::StreamOwn
            },
            payload,
            newline,
            Conduit::Formatting
        );
    }

    /// Names the type and the origin of every position filled by a
    /// binding the tables resolved, leaving the rest untyped.
    ///
    /// Every formatting write goes through here, whichever macro wrote it.
    /// Running it on only the stream macros left the same hole readable
    /// when written with `println!` and unreadable when written with
    /// `writeln!` to a handle — a difference in how the write is spelled,
    /// never in what it says.
    ///
    /// Re-reads the positions of the write recorded last, now that the
    /// values written inside its arguments have records of their own.
    ///
    /// A write is recorded where it stands, before the walk descends
    /// into what it prints; a position filled by a construct — a choice,
    /// a call, an operation — therefore has nothing to name at that
    /// moment. Reading the positions again once the walk has come back
    /// out costs one lookup and turns those positions from unreadable
    /// into what they are.
    /// A hole written as an inline capture spells the name inside the
    /// template rather than as an expression, so no path visit ever
    /// sees it — the use is counted here, where the capture is read.
    pub(super) fn settle_hole_origins(&mut self, invocation: &syn::Macro, skipped: usize) {
        let Some(index) = self.facts.output_writes.len().checked_sub(1) else {
            return;
        };
        let Ok(parsed) = invocation.parse_body_with(
            syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated
        ) else {
            return;
        };
        let written: Vec<&syn::Expr> = parsed.iter().skip(skipped).collect();
        let Some(recorded) = self.facts.output_writes.get(index) else {
            return;
        };
        let settled = self.typed_holes(recorded.payload.clone(), &written);
        let captured: Vec<String> = match &settled {
            Payload::Template(segments) => segments
                .iter()
                .filter_map(|segment| match segment {
                    output::Segment::Hole(hole) => match &hole.source {
                        output::HoleSource::Named(name) => Some(name.clone()),
                        _ => None
                    },
                    output::Segment::Text(_) => None
                })
                .collect(),
            _ => Vec::new()
        };
        if let Some(recorded) = self.facts.output_writes.get_mut(index) {
            recorded.payload = settled;
        }
        for name in captured {
            self.bump_param(&name, false);
        }
    }

    /// A position written as a captured name is answered from the
    /// tables, which hold the binding it names. A position written as an
    /// expression is answered by reading that expression's own origin —
    /// a lookup over records the walk already made, never a new record —
    /// so a field, an element or a call filling a position states where
    /// its value came from exactly as it would anywhere else.
    fn typed_holes(&self, payload: Payload, written: &[&syn::Expr]) -> Payload {
        let Payload::Template(segments) = payload else {
            return payload;
        };
        Payload::Template(
            segments
                .into_iter()
                .map(|segment| match segment {
                    output::Segment::Hole(mut hole) => {
                        match &hole.source {
                            output::HoleSource::Named(name) => {
                                hole.type_name = self.param_types.get(name).cloned();
                                hole.origin = self.named_value(name);
                            }
                            output::HoleSource::Positional(index)
                            | output::HoleSource::Indexed(index) => {
                                if let Some(argument) =
                                    usize::try_from(*index).ok().and_then(|at| written.get(at))
                                {
                                    hole.type_name = self.expr_type(argument);
                                    hole.origin = self.origin_of(argument);
                                }
                            }
                        }
                        output::Segment::Hole(hole)
                    }
                    text @ output::Segment::Text(_) => text
                })
                .collect()
        )
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
            acquisition: Acquisition::Unknown,
            buffering:   output::Buffering::Unknown
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
        let payload = self.typed_holes(
            output::argument_payload(rest),
            rest.get(1..).unwrap_or_default()
        );
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
