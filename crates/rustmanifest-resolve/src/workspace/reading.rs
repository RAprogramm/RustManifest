// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! One file being read, and the two things every reading of it needs.
//!
//! Every question asked below takes the same three values: the semantics
//! answering it, the database the answer is rendered against, and the file
//! the position belongs to. Carrying them as one value is not tidiness — it
//! is what keeps a rendering from being done against the wrong crate, which
//! produces a type name that looks right and names something else.

use ra_ap_hir::{DisplayTarget, HasCrate, HirDisplay, Semantics, Type};
use ra_ap_ide_db::RootDatabase;
use ra_ap_syntax::TextRange;
use rustmanifest_schema::{Location, Range};

/// The file a reading is settling positions in.
///
/// The file's name outlives nothing but the walk over that one file, while
/// the database outlives the whole run — two lifetimes, because tying them
/// together would make the name live as long as the database and the walk
/// could not move on to the next file.
#[derive(Clone, Copy)]
pub(super) struct Reading<'db, 'file> {
    /// Semantics every question is asked of.
    pub(super) semantics: &'db Semantics<'db, RootDatabase>,
    /// Database a type is rendered against.
    pub(super) database:  &'db RootDatabase,
    /// Path the settled positions belong to.
    pub(super) file:      &'file str
}

impl Reading<'_, '_> {
    /// Turns a range in the file being read into a settled location.
    pub(super) fn located(self, span: TextRange) -> Location {
        Location {
            file:  std::path::PathBuf::from(self.file),
            range: Range {
                start: u32::from(span.start()),
                end:   u32::from(span.end())
            }
        }
    }

    /// Renders `typed` the way its own crate spells it, or `None` where the
    /// rendering carries an unknown.
    ///
    /// A rendering containing an unknown is not a partial answer to be
    /// passed on: read further down it is a type name like any other. The
    /// position is left unsettled instead.
    pub(super) fn render(self, typed: &Type<'_>) -> Option<String> {
        let target = DisplayTarget::from_crate(self.database, typed.krate(self.database).into());
        let rendered = typed.display(self.database, target).to_string();
        (!rendered.contains("{unknown}")).then_some(rendered)
    }
}
