// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The shape of a borrow edit: a zero-width insertion at a byte offset.
//!
//! Every edit this fix emits is the same one character `&` placed in front
//! of a type or an argument, so the whole construction side of the fix is
//! one constructor and nothing else.

use std::path::PathBuf;

use rustmanifest_schema::{Applicability, Fix, Range};

/// Builds a zero-width machine-applicable insertion, addressed to
/// `file` or to the finding's own file when `None`.
pub(super) fn insertion(at: usize, text: &str, file: Option<PathBuf>) -> Fix {
    let offset = u32::try_from(at).unwrap_or(u32::MAX);
    Fix {
        file,
        rename_to: None,
        range: Range {
            start: offset,
            end:   offset
        },
        replacement: text.to_owned(),
        applicability: Applicability::MachineApplicable
    }
}
