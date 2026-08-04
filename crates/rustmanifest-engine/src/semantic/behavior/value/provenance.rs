// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Where one value came from, as one body proves it.

use super::{gap::OriginGap, literal::Literal, slot::ParamSlot};

/// Where one value came from, as one body proves it.
///
/// Six positive answers and one refusal. A written literal and an input
/// of this function are leaves: neither is an event, so neither has a
/// site. The other four name a record of the owning function, by channel
/// and by position in it — an index, never an inlined subtree, so two
/// consumers of one value name the same record instead of carrying two
/// copies of it. Unfolding those indices into a tree is the composition
/// tier's work and is bounded there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provenance {
    /// A value written in the source.
    Written(Literal),
    /// The value one of this function's own inputs arrived as. The one
    /// free variable of the term, and the whole of why a summary
    /// composes: a caller substitutes the origin of its own argument for
    /// this slot and is left with a term naming nothing of the callee's.
    Parameter(ParamSlot),
    /// The value the read at this position of the input channel draws.
    Read(u32),
    /// The value the operation at this position of the operation channel
    /// produces.
    Computed(u32),
    /// Whichever value the alternative taken by the choice at this
    /// position of the choice channel produces.
    Chosen(u32),
    /// The value the call at this position of the call-site channel hands
    /// back.
    Called(u32),
    /// Not derived, with the reason.
    NotDerived(OriginGap)
}

impl Provenance {
    /// The channel a positive answer names, as a stable key, or the gap's
    /// own key where the origin was not derived.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Written(_) => "written",
            Self::Parameter(_) => "parameter",
            Self::Read(_) => "read",
            Self::Computed(_) => "computed",
            Self::Chosen(_) => "chosen",
            Self::Called(_) => "called",
            Self::NotDerived(gap) => gap.label()
        }
    }

    /// Whether the origin was settled at all.
    #[must_use]
    pub const fn derived(&self) -> bool {
        !matches!(self, Self::NotDerived(_))
    }
}
