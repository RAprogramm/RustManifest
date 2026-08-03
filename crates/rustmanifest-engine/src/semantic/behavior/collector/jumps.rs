// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The non-local jumps of one kind a construct may perform, and which
//! enclosing construct absorbs each of them.

use std::collections::BTreeSet;

/// The jumps of one kind — `break` or `continue` — a construct may
/// perform, split by what decides which construct absorbs them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Jumps {
    /// Whether an unlabelled jump of this kind may leave the construct;
    /// the nearest enclosing loop absorbs it.
    pub(super) bare:    bool,
    /// Labels named by jumps of this kind, each absorbed by the
    /// construct carrying that label.
    pub(super) labeled: BTreeSet<String>
}

impl Jumps {
    /// The jumps of two constructs taken together, whether they run one
    /// after the other or one instead of the other — either way a jump
    /// of either is a jump of the whole.
    pub(super) fn merged(mut self, other: Self) -> Self {
        self.bare = self.bare || other.bare;
        self.labeled.extend(other.labeled);
        self
    }

    /// Whether a jump of this kind targets the construct carrying
    /// `label`.
    pub(super) fn target(&self, label: Option<&syn::Label>) -> bool {
        self.bare
            || label.is_some_and(|label| self.labeled.contains(&label.name.ident.to_string()))
    }

    /// Drops the jumps the construct carrying `label` absorbs.
    pub(super) fn absorbed_by(&mut self, label: Option<&syn::Label>) {
        self.bare = false;
        if let Some(label) = label {
            self.labeled.remove(&label.name.ident.to_string());
        }
    }
}
