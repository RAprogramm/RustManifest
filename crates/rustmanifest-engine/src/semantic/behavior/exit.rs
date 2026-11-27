// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What the process leaves behind: the status a site hands to whoever
//! started the program.
//!
//! The status is as observable as anything the program writes, and is
//! read the same way — one record per site, in body order, never
//! deduplicated, each carrying what the source proves and nothing beyond
//! it. A shell that branches on the status reads exactly this.
//!
//! Two spellings prove a status. The library's own exit-status type names
//! success and failure outright, and the process-ending call takes the
//! number it will hand back. Where the number is computed rather than
//! written, the site is recorded with the status left unproven: that a
//! status is produced here is a fact, which one it is is not.

use super::output::SiteGuard;

/// The status a site hands back to whoever started the process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExitStatus {
    /// The status the library spells as success.
    Success,
    /// The status the library spells as failure.
    Failure,
    /// A status written as a number in the source.
    Code(u8),
    /// A status produced here, computed rather than written.
    Computed
}

impl std::fmt::Display for ExitStatus {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Success => out.write_str("success"),
            Self::Failure => out.write_str("failure"),
            Self::Code(code) => write!(out, "code {code}"),
            Self::Computed => out.write_str("computed")
        }
    }
}

/// How a site produces its status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExitRoute {
    /// A status value the function hands back to its caller, which ends
    /// the process only where the chain of callers reaches the entry.
    Returned,
    /// The process-ending call, which does not come back.
    Called
}

impl std::fmt::Display for ExitRoute {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Returned => out.write_str("returned"),
            Self::Called => out.write_str("called")
        }
    }
}

/// One observed site producing a process status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessExit {
    /// The status the site proves.
    pub status: ExitStatus,
    /// How the site produces it.
    pub route:  ExitRoute,
    /// Byte range of the construct producing it.
    pub site:   rustmanifest_schema::Range,
    /// What stands between the site and unconditional execution.
    pub guard:  SiteGuard
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

    use super::{ExitRoute, ExitStatus};
    use crate::source::Source;

    fn statuses(text: &str) -> Vec<(ExitStatus, ExitRoute)> {
        let sources = [Source::new(PathBuf::from("src/main.rs"), text.to_owned())];
        crate::semantic::behavior_with(
            &sources,
            Some(&crate::semantic::settling::settled_over(&sources))
        )
        .iter()
        .flat_map(|fact| fact.exits.clone())
        .map(|exit| (exit.status, exit.route))
        .collect()
    }

    /// Both statuses the library names are read where they are written,
    /// each as a value the function hands back.
    #[test]
    fn the_named_statuses_are_read_from_the_source() {
        assert_eq!(
            statuses(
                "use std::process::ExitCode;\nfn main() -> ExitCode {\n    if flag { return \
                 ExitCode::FAILURE; }\n    ExitCode::SUCCESS\n}"
            ),
            vec![
                (ExitStatus::Failure, ExitRoute::Returned),
                (ExitStatus::Success, ExitRoute::Returned)
            ]
        );
    }

    /// The process-ending call takes the number it hands back, and zero
    /// is the success the shell reads it as.
    #[test]
    fn the_process_ending_call_carries_its_written_number() {
        assert_eq!(
            statuses("fn main() {\n    std::process::exit(2);\n}"),
            vec![(ExitStatus::Code(2), ExitRoute::Called)]
        );
        assert_eq!(
            statuses("fn main() {\n    std::process::exit(0);\n}"),
            vec![(ExitStatus::Success, ExitRoute::Called)]
        );
    }

    /// A number the source does not write is not invented: the site is
    /// recorded, the status is not.
    #[test]
    fn a_computed_number_leaves_the_status_unproven() {
        assert_eq!(
            statuses("fn main() {\n    std::process::exit(code());\n}"),
            vec![(ExitStatus::Computed, ExitRoute::Called)]
        );
    }

    /// A constant of the program's own named like the library's is not
    /// the library's, and produces no status.
    #[test]
    fn a_namesake_constant_of_another_type_produces_no_status() {
        assert_eq!(
            statuses("fn main() {\n    let _ = Signal::SUCCESS;\n}"),
            [] as [(
                crate::semantic::behavior::exit::ExitStatus,
                crate::semantic::behavior::exit::ExitRoute
            ); 0]
        );
        assert_eq!(
            statuses("fn main() {\n    other::exit(1);\n}"),
            [] as [(
                crate::semantic::behavior::exit::ExitStatus,
                crate::semantic::behavior::exit::ExitRoute
            ); 0]
        );
    }
}
