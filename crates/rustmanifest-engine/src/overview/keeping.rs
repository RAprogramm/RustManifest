// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The section stating what the program holds and what it does with it.

use std::fmt::Write as _;

use crate::semantic::Workspace;

/// Writes what the program keeps, one sentence per store, or nothing at
/// all where no store was proven.
pub(super) fn keeping_section(out: &mut String, workspace: &Workspace<'_>) {
    let said = crate::semantic::purpose(workspace);
    if said.is_empty() {
        return;
    }
    out.push_str("\n## What it keeps\n\n");
    for sentence in said {
        let _ = writeln!(out, "- {sentence}");
    }
}
