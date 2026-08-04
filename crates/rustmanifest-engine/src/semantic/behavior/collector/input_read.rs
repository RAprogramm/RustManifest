// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Recording one read of a value from outside the process.
//!
//! Two shapes bring a value in. A call whose whole path names a source
//! hands over what the process was started with or started in; a read
//! method on a handle draws from the process's own input stream. Both are
//! recorded here, and both keep the part they draw as far as the source
//! fixes it.

mod handle;

pub(super) use self::handle::matched;
use super::{
    FactCollector,
    catalog::intake::{INPUT_SOURCE_PATHS, READ_METHODS}
};
use crate::semantic::behavior::{
    input::{Channel, InputRead, Intake, ReadShape, Selector, SelectorGap, Terminator},
    output::{Acquisition, Multiplicity},
    value::Provenance
};

/// What one reading construct was read to draw, before the position it
/// stands at is stamped on it.
struct Drawn {
    /// Where the value comes from.
    channel:     Channel,
    /// Which part of that channel is drawn.
    selector:    Selector,
    /// What shape is taken off it.
    shape:       ReadShape,
    /// By what machinery the value reaches the program.
    intake:      Intake,
    /// What becomes of the line terminator.
    terminator:  Terminator,
    /// Where the channel's lock was taken relative to the site.
    acquisition: Acquisition
}

impl FactCollector {
    /// Records one read, answering with its position in the channel.
    fn push_read(&mut self, span: &std::ops::Range<usize>, drawn: Drawn) -> usize {
        let index = self.facts.input_reads.len();
        let site = rustmanifest_schema::Range {
            start: u32::try_from(span.start).unwrap_or(u32::MAX),
            end:   u32::try_from(span.end).unwrap_or(u32::MAX)
        };
        self.facts.input_reads.push(InputRead {
            channel: drawn.channel,
            selector: drawn.selector,
            shape: drawn.shape,
            intake: drawn.intake,
            terminator: drawn.terminator,
            site,
            guard: self.site_guard(),
            conditions: self.site_conditions(),
            multiplicity: Multiplicity::Once,
            acquisition: drawn.acquisition
        });
        self.produced.insert(
            (site.start, site.end),
            Provenance::Read(u32::try_from(index).unwrap_or(u32::MAX))
        );
        index
    }

    /// Records the read a call whose whole path names a source performs.
    pub(super) fn record_source_call(&mut self, call: &syn::ExprCall) -> Option<usize> {
        use syn::spanned::Spanned;

        let syn::Expr::Path(path) = call.func.as_ref() else {
            return None;
        };
        let segments = self.expanded_segments(&path.path);
        let (_, channel, intake, shape, keyed) =
            INPUT_SOURCE_PATHS.iter().find(|(named, _, _, _, _)| {
                named.len() == segments.len() && matched(named, &segments)
            })?;
        let selector = if *keyed {
            key_of(call.args.first())
        } else {
            Selector::Whole
        };
        Some(self.push_read(
            &call.span().byte_range(),
            Drawn {
                channel: *channel,
                selector,
                shape: *shape,
                intake: *intake,
                terminator: Terminator::None,
                acquisition: Acquisition::AtSite
            }
        ))
    }

    /// Records the read a read-trait method on an input handle performs.
    pub(super) fn record_read_method(&mut self, call: &syn::ExprMethodCall) -> Option<usize> {
        use syn::spanned::Spanned;

        let method = call.method.to_string();
        let (_, intake, shape, terminator) = READ_METHODS
            .iter()
            .find(|(named, _, _, _)| *named == method.as_str())?;
        let (channel, acquisition) = self.intake_channel(&call.receiver)?;
        Some(self.push_read(
            &call.span().byte_range(),
            Drawn {
                channel,
                selector: Selector::Whole,
                shape: *shape,
                intake: *intake,
                terminator: *terminator,
                acquisition
            }
        ))
    }
}

/// Which entry of a keyed channel a call names, where the source writes
/// the key as a literal.
fn key_of(argument: Option<&syn::Expr>) -> Selector {
    let Some(written) = argument else {
        return Selector::NotIdentified(SelectorGap::NonLiteralKey);
    };
    let syn::Expr::Lit(literal) = written else {
        return Selector::NotIdentified(SelectorGap::RuntimeValue);
    };
    match &literal.lit {
        syn::Lit::Str(key) => Selector::Named(key.value()),
        _ => Selector::NotIdentified(SelectorGap::NonLiteralKey)
    }
}
