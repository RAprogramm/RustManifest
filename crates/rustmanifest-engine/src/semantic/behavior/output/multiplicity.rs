// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! How many times a site runs per call, kept to the two readings the
//! analysis can actually produce.

/// How many times a site runs per execution of its function.
///
/// Only the two readings the analysis can produce exist: a site that
/// cannot run twice, and a site that runs a number of times no syntax
/// bounds. A counted bound would need the iteration count of the
/// enclosing construct, which this tier does not compute, and a variant
/// nothing constructs is a promise the engine does not keep.
///
/// Standing inside a loop is not the question — completing the loop
/// body is. A body that provably leaves on its first pass runs its
/// writes once however many times the keyword around it suggests, so
/// the reading comes from the completion analysis of that body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Multiplicity {
    /// No more than once per call: nothing repeats the site. Whether it
    /// runs at all is the separate question [`SiteGuard`](crate::SiteGuard)
    /// answers.
    Once,
    /// A loop or cycle bounds it with no static limit.
    Unbounded
}
