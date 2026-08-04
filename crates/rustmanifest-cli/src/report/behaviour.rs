// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The behaviour section: what the program does for an observer outside
//! the process, per entry point, in the words the recorded facts license.

use std::fmt::Write as _;

use rustmanifest_engine::Source;

use super::location::relative;

/// What the program does for an observer outside the process, per entry
/// point, in the words the recorded facts license.
pub(super) fn behaviour(sources: &[Source], root: &std::path::Path) -> String {
    let mut out = String::from("## behaviour\n\n");
    let entries = rustmanifest_engine::program_meaning(sources);
    if entries.is_empty() {
        out.push_str("- no process entry point\n");
        return out;
    }
    for entry in &entries {
        let _ = writeln!(out, "- entry point `{}`", relative(&entry.file, root));
        for claim in &entry.claims {
            let _ = writeln!(
                out,
                "  - {} writes {} to {}{}",
                certainty(claim.execution),
                claim.payload_text(),
                claim.write.stream,
                if claim.write.multiplicity == rustmanifest_engine::Multiplicity::Unbounded {
                    ", once per iteration"
                } else {
                    ""
                }
            );
            for (position, alternatives) in claim.hole_alternatives() {
                let _ = writeln!(
                    out,
                    "    - `<{position}>` is one of {}",
                    alternatives.join(", ")
                );
            }
        }
        let _ = writeln!(
            out,
            "  - {}",
            rustmanifest_engine::completion_rule(entry.completion)
        );
        for claim in &entry.exits {
            let _ = writeln!(
                out,
                "  - {} ends the process with {} in `{}`",
                certainty(claim.execution),
                claim.exit.status,
                claim.function
            );
        }
    }
    out
}

/// How certainly something happens, in one word.
const fn certainty(grade: rustmanifest_engine::Grade) -> &'static str {
    match grade {
        rustmanifest_engine::Grade::Certain => "always",
        rustmanifest_engine::Grade::Possible => "maybe",
        rustmanifest_engine::Grade::Dead => "never",
        rustmanifest_engine::Grade::Unreached => "unreached"
    }
}
