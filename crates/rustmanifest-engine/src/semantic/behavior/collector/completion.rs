// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The lattice a construct's reachability answer lives in, with the two
//! compositions the reachability fold reads it through.

/// Whether the program point after a construct is reached, given that
/// the construct is entered.
///
/// The three values form a lattice, not a scale: `Never` and `Always`
/// are both proofs and are incomparable with each other, `Unknown` is
/// the absence of either proof. Every strong reading this channel emits
/// — "runs on every path", "can never run" — is a claim about all
/// executions, so it is only ever made from a proof; anything this tier
/// cannot see through lands on `Unknown` and weakens the reading instead
/// of strengthening it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Completion {
    /// The point after the construct is never reached: every path
    /// through it leaves by a return, a break, or a divergence.
    Never,
    /// Neither reaching the point after nor leaving before it is proven.
    Unknown,
    /// The point after the construct is always reached.
    #[default]
    Always
}

impl Completion {
    /// Sequential composition: what holds after running `next` where
    /// `self` completes. A divergence anywhere in the sequence carries
    /// through, an unproven step leaves the whole sequence unproven.
    pub(super) const fn then(self, next: Self) -> Self {
        match (self, next) {
            (Self::Never, _) | (_, Self::Never) => Self::Never,
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            (Self::Always, Self::Always) => Self::Always
        }
    }

    /// Branch merge: a proof survives only when every branch proves the
    /// same thing, which is what makes a divergence on all arms of an
    /// exhaustive construct kill what follows and a divergence on one
    /// arm merely weaken it.
    pub(super) const fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::Never, Self::Never) => Self::Never,
            (Self::Always, Self::Always) => Self::Always,
            _ => Self::Unknown
        }
    }
}
