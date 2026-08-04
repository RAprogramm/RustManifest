// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What a call site hands its callee.

use super::provenance::Provenance;

/// What a call site hands its callee.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Arguments {
    /// Origins of the written arguments, in written order, the receiver
    /// first where the call has one.
    Written(Vec<Provenance>),
    /// A site recorded for a reason other than a written call — the error
    /// conversion a `?` performs, the poll an `await` stands for, the
    /// inner call a service wrapper dispatches to — whose arguments are
    /// written nowhere to be read. An empty list would read as a call of
    /// no arguments, which is a different fact.
    Unwritten
}

impl Arguments {
    /// The origins written at this site, or nothing where none are
    /// written anywhere.
    #[must_use]
    pub fn written(&self) -> Option<&[Provenance]> {
        match self {
            Self::Written(origins) => Some(origins),
            Self::Unwritten => None
        }
    }
}
