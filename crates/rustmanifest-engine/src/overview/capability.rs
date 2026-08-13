// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What each module can do to the world outside the process, and which of
//! those the program starts.
//!
//! Every line cites the call it rests on, so the section is checkable
//! rather than merely readable. A module that proved nothing is absent, not
//! printed as empty: absence here states "nothing was proved", which is a
//! different claim from "nothing happens".

use std::collections::BTreeMap;

use super::fragment::push;
use crate::semantic::{Workspace, capabilities, started};

/// Prints the per-module capabilities and the set an entry point starts.
pub(super) fn capability_section(out: &mut String, workspace: &Workspace<'_>, top: usize) {
    push(out, "## Capabilities\n\n");
    let rows = capabilities(workspace);
    if rows.is_empty() {
        push(
            out,
            "- no module was proved to touch anything outside the process\n\n"
        );
        return;
    }
    let mut by_module: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for row in &rows {
        by_module
            .entry(row.module.as_str())
            .or_default()
            .push(format!(
                "{} ({})",
                row.proven.capability, row.proven.evidence.call
            ));
    }
    for (module, verbs) in by_module.iter().take(top) {
        push(out, &format!("- `{module}`: {}\n", verbs.join(", ")));
    }
    if by_module.len() > top {
        push(
            out,
            &format!("- ... and {} more modules\n", by_module.len() - top)
        );
    }
    started_lines(out, workspace);
}

/// Prints which capabilities an entry point reaches, and which are proved
/// but unreached.
fn started_lines(out: &mut String, workspace: &Workspace<'_>) {
    let rows = started(workspace);
    let reached: Vec<String> = rows
        .iter()
        .filter(|row| row.reached)
        .map(|row| row.proven.capability.to_string())
        .collect();
    let unreached: Vec<String> = rows
        .iter()
        .filter(|row| !row.reached)
        .map(|row| row.proven.capability.to_string())
        .collect();
    if reached.is_empty() {
        push(out, "- no entry point reaches any of them\n");
    } else {
        push(out, &format!("- an entry starts: {}\n", reached.join(", ")));
    }
    if !unreached.is_empty() {
        push(
            out,
            &format!(
                "- proved but not reached from an entry: {}\n",
                unreached.join(", ")
            )
        );
    }
    push(out, "\n");
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

    use super::*;
    use crate::{
        semantic::{module_graph::declared_tree, settling::settle_identity},
        source::Source
    };

    /// The section rendered over one file, with the module tree and the
    /// call identity a compiler settles for it.
    ///
    /// An entry reaches a capability through the call graph, and a call
    /// binds to the body the resolution names it reaches — so a fixture
    /// asking what an entry starts has to state that identity first.
    fn rendered(path: &str, text: &str) -> String {
        let sources = vec![Source::new(PathBuf::from(path), text.to_owned())];
        let mut settled = declared_tree(&sources);
        settle_identity(
            &mut settled,
            &sources,
            &crate::semantic::behavior_with(&sources, Some(&crate::semantic::settling::settled_over(&sources)))
        );
        let mut out = String::new();
        capability_section(&mut out, &Workspace::resolved(&sources, &settled), 10);
        out
    }

    #[test]
    fn a_module_line_names_the_verb_and_the_call_behind_it() {
        let out = rendered(
            "app/src/main.rs",
            "fn serve() { std::net::TcpListener::bind(\"0\").ok(); }\nfn main() { serve(); }"
        );
        assert!(out.contains("- `crate`: serves (std::net::TcpListener::bind)"));
        assert!(out.contains("- an entry starts: serves"));
    }

    #[test]
    fn a_capability_no_entry_reaches_is_named_as_such() {
        let out = rendered(
            "app/src/main.rs",
            "fn serve() { std::net::TcpListener::bind(\"0\").ok(); }\nfn main() { }"
        );
        assert!(out.contains("- proved but not reached from an entry: serves"));
        assert!(out.contains("- no entry point reaches any of them"));
    }

    #[test]
    fn a_workspace_proving_nothing_says_so_once() {
        let out = rendered("app/src/lib.rs", "pub fn add(a: u8, b: u8) -> u8 { a + b }");
        assert!(out.contains("no module was proved to touch anything outside the process"));
        assert!(!out.contains("an entry starts"));
    }
}
