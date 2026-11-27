// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The structure section: what the sources hold, rendered from the
//! overview and stripped of the lines that only state an absence.

use std::fmt::Write as _;

use rustmanifest_engine::{
    overview::{overview_markdown, Convention},
    Workspace,
};
use rustmanifest_learn::{learn_roles, LearnOptions};

/// What the sources hold: modules, types, roles, boundaries, layers, the
/// external surface, and the learned conventions.
pub(super) fn structure(workspace: &Workspace<'_>, top: usize) -> String {
    let conventions: Vec<Convention> = learn_roles(workspace, LearnOptions::default())
        .into_iter()
        .map(|clause| Convention {
            role: clause.role,
            includes: clause.includes,
            excludes: clause.excludes,
        })
        .collect();
    let rendered = overview_markdown(workspace, top, &conventions);
    let mut out = String::from("\n## structure\n");
    for block in rendered.split("\n## ").skip(1) {
        let Some((heading, body)) = block.split_once('\n') else {
            continue;
        };
        let lines: Vec<&str> = body
            .lines()
            .filter(|line| line.starts_with("- "))
            .filter(|line| !empty_note(line))
            .collect();
        if lines.is_empty() {
            continue;
        }
        let _ = write!(out, "\n### {}\n\n{}\n", heading.trim(), lines.join("\n"));
    }
    out
}

/// Whether a rendered line states an absence rather than a fact. A
/// report that prints every absence buries the few lines that carry
/// content, so absences are dropped here and the coverage section says
/// what was and was not read.
fn empty_note(line: &str) -> bool {
    let text = line.trim_start_matches("- ");
    text.starts_with("no ") || text.starts_with("nothing ")
}
