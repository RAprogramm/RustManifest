// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The analyzed sources together with what a resolver settled about them.
//!
//! The two travel as one value because neither is useful alone here: a
//! resolution is keyed by file and position, so reading it needs the
//! sources to turn a fact's source index into a path. Holding them apart
//! made three different structures declare the same pair.

use rustmanifest_schema::{Location, Range, Resolution};

use crate::{semantic::behavior::FunctionFacts, source::Source};

/// What a resolver settled about the analyzed sources, looked up by the
/// position a call is written at.
///
/// Carried through the unfolding rather than consulted at the end because
/// the position is only known here: by the time a term is rendered, the
/// site it came from is no longer part of it.
#[derive(Debug, Clone, Copy)]
pub(in crate::semantic) struct Settled<'a> {
    /// What the resolver settled, where one ran at all.
    resolution: Option<&'a Resolution>,
    /// Sources, so a fact's source index becomes the path a resolution
    /// is keyed by.
    sources:    &'a [Source]
}

impl<'a> Settled<'a> {
    /// Takes what a resolver settled about `sources`, or nothing.
    pub(in crate::semantic) const fn new(
        resolution: Option<&'a Resolution>,
        sources: &'a [Source]
    ) -> Self {
        Self {
            resolution,
            sources
        }
    }

    /// Returns the type the call written at `site` of `fact`'s source hands
    /// back, where a resolver settled it.
    pub(in crate::semantic) fn returns(self, fact: &FunctionFacts, site: Range) -> Option<String> {
        let path = self.sources.get(fact.source)?.path().to_path_buf();
        let settled = self.resolution?.at(&Location {
            file:  path,
            range: site
        })?;
        Some(settled.returns.clone())
    }
}

impl<'a> Settled<'a> {
    /// Returns the sources this was taken over.
    pub(in crate::semantic) const fn sources(self) -> &'a [Source] {
        self.sources
    }

    /// Returns what a resolver settled, where one ran.
    pub(in crate::semantic) const fn resolution(self) -> Option<&'a Resolution> {
        self.resolution
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use std::path::PathBuf;

    use rustmanifest_schema::ResolvedCall;

    use super::*;

    #[test]
    fn nothing_is_answered_where_no_resolver_ran() {
        let sources = [Source::new(
            PathBuf::from("src/lib.rs"),
            "fn go() {}".to_owned()
        )];
        let settled = Settled::new(None, &sources);
        assert!(settled.resolution().is_none());
        assert_eq!(settled.sources().len(), 1);
    }

    #[test]
    fn a_settled_position_answers_with_the_type_it_hands_back() {
        let sources = [Source::new(
            PathBuf::from("src/lib.rs"),
            "fn go() {}".to_owned()
        )];
        let resolution = Resolution {
            resolver: "test".to_owned(),
            calls: vec![ResolvedCall {
                at:       Location {
                    file:  PathBuf::from("src/lib.rs"),
                    range: Range {
                        start: 3, end: 5
                    }
                },
                callee:   "is_some".to_owned(),
                returns:  "bool".to_owned(),
                receiver: None,
                receiver_path: None,
                reaches:  None,
                takes:    None
            }],
            ..Resolution::default()
        };
        let settled = Settled::new(Some(&resolution), &sources);
        let facts = crate::semantic::behavior::behavior(&sources);
        let fact = facts.first().expect("the source records one function");
        assert_eq!(
            settled.returns(
                fact,
                Range {
                    start: 3, end: 5
                }
            ),
            Some("bool".to_owned())
        );
        assert_eq!(
            settled.returns(
                fact,
                Range {
                    start: 9, end: 9
                }
            ),
            None
        );
    }
}
