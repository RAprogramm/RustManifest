// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What one construct proves about the program point after it, together
//! with the non-local exits it may perform on the way there, and the
//! compositions that carry those answers through a body.

use super::{completion::Completion, jumps::Jumps};

/// What one construct proves about the program point after it, together
/// with the non-local exits it may perform on the way there.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Flow {
    /// Whether the point after the construct is reached.
    pub(super) completion:   Completion,
    /// Whether a `return` or a `?` inside may leave the function.
    pub(super) returns:      bool,
    /// `break`s inside that leave an enclosing loop or labelled block.
    pub(super) breaks:       Jumps,
    /// `continue`s inside that hand control back to the top of an
    /// enclosing loop.
    pub(super) continues:    Jumps,
    /// Whether the construct holds something this tier cannot see
    /// through that may itself carry a `break` or a `continue` — an
    /// unexpanded macro, a token stream no rule of this tier reads.
    ///
    /// It is tracked apart from [`Completion::Unknown`] because the two
    /// answer different questions: a call may diverge and still cannot
    /// jump out of the caller's loop, while a macro may expand to a
    /// bare `break`. Conflating them would leave every opaque call
    /// unbounding a loop that nothing breaks out of.
    pub(super) opaque_jumps: bool
}

impl Flow {
    /// A construct that completes and leaves nothing behind.
    pub(super) fn settled() -> Self {
        Self::default()
    }

    /// A construct proving `completion` and performing no exit.
    pub(super) fn proving(completion: Completion) -> Self {
        Self {
            completion,
            ..Self::default()
        }
    }

    /// A construct this tier cannot see through: it proves neither that
    /// the point after it is reached nor that it is not, and whatever it
    /// expands to may carry a `break` or a `continue` out of it.
    pub(super) fn opaque() -> Self {
        Self {
            completion: Completion::Unknown,
            opaque_jumps: true,
            ..Self::default()
        }
    }

    /// Sequential composition: `next` runs where `self` completes, so a
    /// construct that never completes hides everything after it.
    pub(super) fn then(self, next: Self) -> Self {
        if matches!(self.completion, Completion::Never) {
            return self;
        }
        Self {
            completion:   self.completion.then(next.completion),
            returns:      self.returns || next.returns,
            breaks:       self.breaks.merged(next.breaks),
            continues:    self.continues.merged(next.continues),
            opaque_jumps: self.opaque_jumps || next.opaque_jumps
        }
    }

    /// Branch merge: control takes one of the two, so an exit either may
    /// perform is possible and only a shared proof survives.
    pub(super) fn join(self, other: Self) -> Self {
        Self {
            completion:   self.completion.join(other.completion),
            returns:      self.returns || other.returns,
            breaks:       self.breaks.merged(other.breaks),
            continues:    self.continues.merged(other.continues),
            opaque_jumps: self.opaque_jumps || other.opaque_jumps
        }
    }

    /// Weakens a proof of divergence into "not proven either way" and
    /// leaves a proof of completion standing — what a construct that
    /// exists in some builds and not others proves in every build.
    pub(super) const fn weakened(mut self) -> Self {
        self.completion = self.completion.join(Completion::Always);
        self
    }

    /// Whether the construct may hand control somewhere that skips the
    /// program point after it.
    pub(super) fn may_skip(&self) -> bool {
        self.returns
            || self.breaks.bare
            || !self.breaks.labeled.is_empty()
            || self.continues.bare
            || !self.continues.labeled.is_empty()
    }

    /// Whether a `break` inside targets the loop that carries `label`.
    pub(super) fn breaks_out_of(&self, label: Option<&syn::Label>) -> bool {
        self.breaks.target(label)
    }

    /// Whether a `continue` inside restarts the loop that carries
    /// `label`, so the loop can begin another iteration even where the
    /// end of its body is never reached.
    pub(super) fn continues_into(&self, label: Option<&syn::Label>) -> bool {
        self.continues.target(label)
    }

    /// Drops the jumps a loop absorbs: the unlabelled breaks and
    /// continues, those naming the loop's own label, and the unproven
    /// ones an opaque construct in its body may carry — a bare jump
    /// hidden inside a macro targets the loop that encloses it.
    pub(super) fn absorbed(mut self, label: Option<&syn::Label>) -> Self {
        self.breaks.absorbed_by(label);
        self.continues.absorbed_by(label);
        self.opaque_jumps = false;
        self
    }
}
