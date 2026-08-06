// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a call to one function hands back.

use super::composed::ComposedOrigin;

/// Version of the composition this tier performs. Stamped into every
/// summary, so a refusal states which reading produced it.
pub const SUMMARY_SET: u32 = 1;

/// What a call to one function hands back, in terms a caller can restate.
///
/// The only free variables of [`Self::result`] are this function's own
/// inputs. That is the whole of why a summary composes: a caller
/// substitutes the origin of its own argument for the input at that
/// position and is left with a term naming no name of the callee's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionSummary {
    /// Qualified name of the function.
    pub function:    String,
    /// Index of the fact the summary was derived from.
    pub fact:        usize,
    /// One entry per declared input, the receiver first, so substitution
    /// aligns by position.
    pub slots:       Vec<Option<String>>,
    /// Where the value handed back comes from.
    ///
    /// Several return positions become one choice; none at all becomes a
    /// refusal, which states that the reading found no return position
    /// and not that the function hands nothing back.
    pub result:      ComposedOrigin,
    /// Where the value handed back *instead* comes from, for a function
    /// that hands back a failure half of its own.
    ///
    /// Kept apart from the result because a caller reads one or the
    /// other and never both: the half it takes is written in the pattern
    /// it takes it with.
    pub failure:     ComposedOrigin,
    /// Version of the composition that produced it.
    pub summary_set: u32
}
