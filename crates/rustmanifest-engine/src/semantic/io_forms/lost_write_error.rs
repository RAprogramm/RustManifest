// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The entry point that writes through the print family while returning
//! nothing, so a failed write becomes a panic rather than an exit code.

use super::{
    catalogue::{WRITE_ERROR, library_knowledge, support},
    finding::{detail, trace},
    shipped_code::shipped
};
use crate::semantic::{
    behavior::Conduit, entry::entry_points, insight::Insight, workspace::Workspace
};

/// Reports an entry point that writes through the print family while
/// returning nothing, so a failed write becomes a panic rather than an
/// exit code.
///
/// The panic belongs to the print family alone: it is that machinery that
/// turns the write's failed result into one. A byte write hands its
/// failure back as a value, and an entry point performing only those
/// loses nothing by returning nothing — it is charged for no panic that
/// its code cannot reach.
pub(in crate::semantic) fn lost_write_error_insights(workspace: &Workspace<'_>) -> Vec<Insight> {
    let sources = workspace.sources();
    let library = library_knowledge();
    let facts = workspace.facts();
    let mut found = Vec::new();
    for entry in entry_points(sources, facts) {
        let Some(fact) = facts.get(entry.fact) else {
            continue;
        };
        if entry.returns || !shipped(sources, fact) {
            continue;
        }
        let Some((write, derived)) = fact
            .output_writes
            .iter()
            .filter(|write| write.conduit == Conduit::Formatting)
            .find_map(|write| support(&WRITE_ERROR, write.stream, library).map(|of| (write, of)))
        else {
            continue;
        };
        found.push(Insight::new(
            fact.source,
            write.site,
            detail(
                &WRITE_ERROR,
                &fact.function,
                &format!(
                    "the process entry point writes to {} and declares no return type",
                    write.stream
                ),
                1
            ),
            trace(
                &format!("`{}` declares no return type", fact.function),
                "the entry signature returns nothing, so no failure can leave it",
                derived
            )
        ));
    }
    found
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use std::path::PathBuf;

    use super::lost_write_error_insights;
    use crate::source::Source;

    fn sources(text: &str) -> Vec<Source> {
        vec![Source::new(PathBuf::from("src/main.rs"), text.to_owned())]
    }

    /// The insights of a fixture read against what a compiler settles
    /// about it: the standard macros it writes and the types it
    /// declares, without which the readings above answer nothing.
    fn insights_over(sources: &[Source]) -> Vec<crate::semantic::insight::Insight> {
        let settled = crate::semantic::settling::settled_over(sources);
        lost_write_error_insights(&crate::semantic::workspace::Workspace::resolved(sources, &settled))
    }

    #[test]
    fn an_entry_point_losing_the_write_error_is_reported() {
        let lost = insights_over(&sources(
            "fn main() {\n    println!(\"hi\");\n}"
        ));
        assert_eq!(lost.len(), 1);
        assert!(lost[0].detail.contains("declares no return type"));

        let returned = insights_over(&sources(
            "fn main() -> std::io::Result<()> {\n    println!(\"hi\");\n    Ok(())\n}"
        ));
        assert!(returned.is_empty());
    }
}
