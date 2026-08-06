// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! How a run ends, said beside what it was for.
//!
//! The statuses a program can leave are already recorded, one per exit
//! site, each with how certainly it runs. What this reading adds is only
//! the joining: the statuses an entry can end with, told apart from one
//! another and named once each, so a sentence about what a program keeps
//! can end where the run does.

use crate::semantic::{
    behavior::{ExitStatus, Grade},
    workspace::Workspace
};

/// One way a run can end.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ending {
    /// Function holding the exit.
    pub function: String,
    /// The status it leaves.
    pub status:   ExitStatus,
    /// Whether every path through the program reaches it.
    pub certain:  bool
}

/// Every way the program's entry points can end, in the order the
/// bodies write them.
#[must_use]
pub fn endings(workspace: &Workspace<'_>) -> Vec<Ending> {
    let mut ways = Vec::new();
    for entry in crate::semantic::program_meaning(workspace) {
        for claim in &entry.exits {
            if claim.execution == Grade::Unreached {
                continue;
            }
            let ending = Ending {
                function: claim.function.clone(),
                status:   claim.exit.status,
                certain:  claim.execution == Grade::Certain
            };
            if !ways.contains(&ending) {
                ways.push(ending);
            }
        }
    }
    ways
}
