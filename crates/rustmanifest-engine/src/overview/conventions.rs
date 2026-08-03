// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Learned conventions as plain renderable data, and the section that
//! prints them. Clauses arrive from the caller, so nothing here knows the
//! learning layer; an empty slice means nothing was learned and the
//! section says so.

use super::fragment::{push, truncation_note};

/// One learned convention clause, as plain renderable data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Convention {
    /// Role the clause votes for.
    pub role:     String,
    /// Feature names that must be present.
    pub includes: Vec<String>,
    /// Feature names that must be absent.
    pub excludes: Vec<String>
}

/// Prints the conventions the learning layer extracted.
pub(super) fn convention_section(out: &mut String, conventions: &[Convention], top: usize) {
    out.push_str("## Learned conventions\n\n");
    if conventions.is_empty() {
        out.push_str("- nothing learned (too few functions)\n");
        return;
    }
    for clause in conventions.iter().take(top) {
        let mut parts: Vec<String> = clause
            .includes
            .iter()
            .map(|name| format!("`{name}`"))
            .collect();
        parts.extend(clause.excludes.iter().map(|name| format!("NOT `{name}`")));
        push(
            out,
            &format!("- {} <= IF {}\n", clause.role, parts.join(" AND "))
        );
    }
    truncation_note(out, conventions.len(), top);
}
