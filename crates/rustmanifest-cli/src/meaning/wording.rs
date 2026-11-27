// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The words both renderings use for an engine grade and for the way an
//! entry point is declared, so the table and the data form never disagree.

use rustmanifest_engine::{EntryKind, Grade};

/// How certainly a write happens, in one word.
pub(super) const fn certainty(grade: Grade) -> &'static str {
    match grade {
        Grade::Certain => "always",
        Grade::Possible => "maybe",
        Grade::Dead => "never",
        Grade::Unreached => "unreached"
    }
}

/// How an entry point is declared, in one phrase.
pub(super) const fn entry_kind(kind: EntryKind) -> &'static str {
    match kind {
        EntryKind::Plain => "entry point",
        EntryKind::Runtime => "entry point behind a runtime attribute",
        EntryKind::Conditional => "entry point behind a configuration gate"
    }
}
