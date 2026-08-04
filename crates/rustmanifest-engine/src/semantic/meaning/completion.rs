// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What the process leaves behind, as a statement about one entry point.
//!
//! Two things decide it, and they are stated separately because they are
//! proven separately. The entry's signature decides the rule the library
//! applies — reaching the end of a returnless entry is a success, a
//! returned status is handed back as it stands, a returned result becomes
//! success or failure by the library's own conversion. The recorded sites
//! decide which statuses the code can actually produce, each with what
//! stands over it and how certainly the call chain reaches it.
//!
//! A site the chain never reaches keeps its claim and says so. Dropping
//! it would turn a fact about the call graph into an empty statement
//! about the program.

use crate::semantic::{
    behavior::{Grade, ProcessExit},
    entry::Completion
};

/// One statement about a status the process can leave behind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitClaim {
    /// Function holding the site.
    pub function:  String,
    /// File the site lives in.
    pub file:      String,
    /// The recorded site.
    pub exit:      ProcessExit,
    /// How certainly the owning function is reached from the entry.
    pub reach:     Grade,
    /// How certainly the site runs, both parts composed.
    pub execution: Grade
}

/// The rule the entry's own signature fixes, in words.
#[must_use]
pub const fn completion_rule(completion: Completion) -> &'static str {
    match completion {
        Completion::Nothing => {
            "the entry returns nothing, so reaching its end ends the process with success"
        }
        Completion::Status => {
            "the entry returns a status, so the process ends with whichever status it hands back"
        }
        Completion::Fallible => {
            "the entry returns a result, so the process ends with success on the ok value and \
             with a failure the library also reports on the error"
        }
    }
}

impl ExitClaim {
    /// What this claim states, in one sentence.
    #[must_use]
    pub fn sentence(&self) -> String {
        format!(
            "{} {} with {}{}{}",
            self.opening(),
            self.route(),
            self.exit.status,
            self.origin(),
            self.weakening()
        )
    }

    /// The opening words, fixed by how certainly the site runs.
    const fn opening(&self) -> &'static str {
        match self.execution {
            Grade::Certain => "the process unconditionally ends",
            Grade::Possible => "the process may end",
            Grade::Dead => "a site that can never run would end the process",
            Grade::Unreached => {
                "a site no call chain from this entry reaches would end the process"
            }
        }
    }

    /// How the status leaves: handed back to a caller, or taken by the
    /// process-ending call itself.
    const fn route(&self) -> &'static str {
        match self.exit.route {
            crate::semantic::ExitRoute::Returned => "by a returned status",
            crate::semantic::ExitRoute::Called => "by the process-ending call"
        }
    }

    /// Where the site sits, named so the sentence can be checked.
    fn origin(&self) -> String {
        format!(" in `{}` at {}", self.function, self.file)
    }

    /// What stands between this claim and certainty.
    fn weakening(&self) -> String {
        if self.execution == Grade::Certain {
            return String::new();
        }
        let mut reasons = Vec::new();
        if self.reach != Grade::Certain {
            reasons.push("a call chain that does not certainly reach it".to_owned());
        }
        if !self.exit.guard.guards.is_empty() {
            reasons.push(format!(
                "{} construct(s) standing over it",
                self.exit.guard.guards.len()
            ));
        }
        if reasons.is_empty() {
            return String::new();
        }
        format!(", weakened by {}", reasons.join(" and "))
    }
}
