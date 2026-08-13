// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Writing down what a compiler settled about a written fixture.
//!
//! Every reading above answers from a resolution and from nothing else, so
//! a test that states an expectation has to state the compiler's answer
//! first. Spelling that out by hand — a location, a byte range, a rendered
//! type — buries the expectation under bookkeeping; this builder names the
//! written fragment instead and works the range out from the text.
//!
//! The fragment is found by its first occurrence, so a fixture wanting a
//! later one says where to look from with [`Settling::after`].

use std::path::PathBuf;

use rustmanifest_schema::{
    Location, Range, ResolvedCall, ResolvedField, ResolvedType, Resolution
};

/// A resolution being written down over one or more fixture files.
pub(in crate::semantic) struct Settling<'text> {
    /// File the fragments named next are written in.
    file:   PathBuf,
    /// Text of that file.
    text:   &'text str,
    /// Offset the next fragment is searched from.
    from:   usize,
    /// What has been settled so far.
    settled: Resolution
}

impl<'text> Settling<'text> {
    /// Starts a resolution over `text`, recorded as the file at `file`.
    pub(in crate::semantic) fn over(file: &str, text: &'text str) -> Self {
        Self {
            file:    PathBuf::from(file),
            text,
            from:    0,
            settled: Resolution {
                resolver: "test".to_owned(),
                ..Resolution::default()
            }
        }
    }

    /// Continues over a second file, from its start.
    pub(in crate::semantic) fn and_over(mut self, file: &str, text: &'text str) -> Self {
        self.file = PathBuf::from(file);
        self.text = text;
        self.from = 0;
        self
    }

    /// Searches the fragments named next from after `written`.
    pub(in crate::semantic) fn after(mut self, written: &str) -> Self {
        self.from = self.located(written).range.end as usize;
        self
    }

    /// States the type the compiler settled for the written `fragment`.
    pub(in crate::semantic) fn typed(mut self, fragment: &str, rendered: &str) -> Self {
        let at = self.located(fragment);
        self.settled.types.push(ResolvedType {
            at,
            rendered: rendered.to_owned()
        });
        self
    }

    /// States the type holding the field written at `fragment` and the
    /// type that field itself has.
    pub(in crate::semantic) fn field(
        mut self,
        fragment: &str,
        owner: &str,
        produced: &str
    ) -> Self {
        let at = self.located(fragment);
        self.settled.fields.push(ResolvedField {
            at,
            owner: owner.to_owned(),
            produced: produced.to_owned()
        });
        self
    }

    /// States the call the compiler settled at `fragment`: the function
    /// it reaches, the type it hands back, and the type of the receiver
    /// where it has one.
    pub(in crate::semantic) fn call(
        mut self,
        fragment: &str,
        callee: &str,
        returns: &str,
        receiver: Option<&str>
    ) -> Self {
        let at = self.located(fragment);
        self.settled.calls.push(ResolvedCall {
            at,
            callee: callee.to_owned(),
            returns: returns.to_owned(),
            receiver: receiver.map(ToOwned::to_owned),
            reaches: None,
            takes: None
        });
        self
    }

    /// Hands back the resolution, in the order every lookup bisects.
    pub(in crate::semantic) fn done(mut self) -> Resolution {
        self.settled.settle();
        self.settled
    }

    /// Where a written fragment stands in the file being settled.
    fn located(&self, fragment: &str) -> Location {
        let start = self
            .text
            .get(self.from..)
            .and_then(|rest| rest.find(fragment).map(|at| self.from + at))
            .unwrap_or_else(|| {
                panic!("the fragment {fragment:?} is written in the fixture from here on")
            });
        Location {
            file:  self.file.clone(),
            range: Range {
                start: u32::try_from(start).unwrap_or(u32::MAX),
                end:   u32::try_from(start + fragment.len()).unwrap_or(u32::MAX)
            }
        }
    }
}
