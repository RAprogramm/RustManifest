// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Which capabilities a program actually starts.
//!
//! A workspace can hold a module that serves a socket and never call it.
//! The call graph separates the two: a capability is *started* when a path
//! of resolved calls runs from an entry point to the function that proved
//! it.
//!
//! The separation is only as good as the call graph, which drops what it
//! cannot resolve. A capability the graph cannot reach is therefore
//! reported as unreached, not as absent — the row stays, and says which of
//! the two it is.

use std::collections::BTreeSet;

use super::{Capability, classify, evidence::Evidence};
use crate::semantic::{entry::entry_points, workspace::Workspace};

/// One capability of a program, and whether an entry point reaches it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Started {
    /// What the program can do.
    pub capability: Capability,
    /// Whether a path of resolved calls runs from an entry point to the
    /// function that proved it.
    pub reached:    bool,
    /// The function and call that proved the capability.
    pub evidence:   Evidence
}

/// Derives every capability of `workspace` with whether an entry point
/// reaches it, sorted by capability.
///
/// A capability proved in several places is still one capability, and a
/// reached proof replaces an unreached one: a single path from an entry is
/// enough to start it, so the row cites the proof that carries that path.
pub(super) fn started(workspace: &Workspace<'_>) -> Vec<Started> {
    let facts = workspace.facts();
    let reachable = reachable_facts(workspace);
    let mut rows: Vec<Started> = Vec::new();
    for (index, fact) in facts.iter().enumerate() {
        for site in &fact.effect_sites {
            let Some(capability) = classify::of(site) else {
                continue;
            };
            let reached = reachable.contains(&index);
            match rows.iter_mut().find(|row| row.capability == capability) {
                Some(row) if reached && !row.reached => {
                    row.reached = true;
                    row.evidence = Evidence::of(fact, site);
                }
                Some(_) => {}
                None => rows.push(Started {
                    capability,
                    reached,
                    evidence: Evidence::of(fact, site)
                })
            }
        }
    }
    rows.sort_by_key(|row| row.capability);
    rows
}

/// Returns the fact indices a resolved call path reaches from any entry
/// point, the entry points themselves included.
fn reachable_facts(workspace: &Workspace<'_>) -> BTreeSet<usize> {
    let facts = workspace.facts();
    let graph = workspace.calls();
    let mut seen = BTreeSet::new();
    let mut pending: Vec<usize> = entry_points(workspace.sources(), facts)
        .into_iter()
        .map(|entry| entry.fact)
        .collect();
    while let Some(index) = pending.pop() {
        if !seen.insert(index) {
            continue;
        }
        pending.extend(graph.callees(index).iter().copied());
    }
    seen
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
    use crate::source::Source;

    fn binary(text: &str) -> Vec<Source> {
        vec![Source::new(
            PathBuf::from("app/src/main.rs"),
            text.to_owned()
        )]
    }

    #[test]
    fn a_capability_the_entry_calls_is_started() {
        let sources = binary(
            "fn serve() { std::net::TcpListener::bind(\"0\").ok(); }\nfn main() { serve(); }"
        );
        let rows = started(&Workspace::new(&sources));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].capability, Capability::Serves);
        assert!(rows[0].reached);
        assert_eq!(rows[0].evidence.function, "serve");
    }

    #[test]
    fn a_capability_no_entry_calls_is_kept_and_marked_unreached() {
        let sources =
            binary("fn serve() { std::net::TcpListener::bind(\"0\").ok(); }\nfn main() { }");
        let rows = started(&Workspace::new(&sources));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].capability, Capability::Serves);
        assert!(!rows[0].reached);
    }

    #[test]
    fn a_reached_proof_outranks_an_unreached_one() {
        let sources = binary(
            "fn cold() { std::env::var(\"A\").ok(); }\nfn warm() { std::env::var(\"B\").ok(); \
             }\nfn main() { warm(); }"
        );
        let rows = started(&Workspace::new(&sources));
        assert_eq!(rows.len(), 1);
        assert!(rows[0].reached);
        assert_eq!(rows[0].evidence.function, "warm");
    }

    #[test]
    fn a_workspace_without_an_entry_point_reaches_nothing() {
        let sources = vec![Source::new(
            PathBuf::from("app/src/lib.rs"),
            "pub fn serve() { std::net::TcpListener::bind(\"0\").ok(); }".to_owned()
        )];
        let rows = started(&Workspace::new(&sources));
        assert_eq!(rows.len(), 1);
        assert!(!rows[0].reached);
    }
}
