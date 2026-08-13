// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What each module of a workspace is for, and which of those the program
//! actually starts.
//!
//! The behavioral tier records what every function touches and, since the
//! effect-site channel, the written path that proved it. This module folds
//! that per function into a statement per module — this one answers a
//! socket, that one reaches a database, that one reads the environment —
//! and every statement carries the function and the call it rests on, so a
//! reader can go to the byte range and check rather than believe.
//!
//! Nothing is inferred that a site did not prove. A module whose functions
//! touch nothing outside the process gets no row at all; silence here means
//! "no proof", never "no capability".

mod classify;
mod evidence;
mod kind;
mod reach;

pub use self::{
    evidence::Evidence,
    kind::{Capability, ProvenCapability},
    reach::Started
};
use super::workspace::Workspace;

/// One module and one thing it can do, with the site that proved it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleCapability {
    /// Module path the capability belongs to, `crate` for a crate root.
    pub module: String,
    /// What the module can do, and what proved it.
    pub proven: ProvenCapability
}

/// Derives the capability of every module of `workspace`.
///
/// One row per (module, capability) pair, keeping the first proving site in
/// source order so the output is deterministic. Rows are sorted by module,
/// then by capability.
#[must_use]
pub fn capabilities(workspace: &Workspace<'_>) -> Vec<ModuleCapability> {
    let module_of = evidence::modules_by_source(workspace);
    let mut rows: Vec<ModuleCapability> = Vec::new();
    for fact in workspace.facts() {
        let module = module_of(fact.source);
        for site in &fact.effect_sites {
            let Some(capability) = classify::of(site) else {
                continue;
            };
            if rows
                .iter()
                .any(|row| row.module == module && row.proven.capability == capability)
            {
                continue;
            }
            rows.push(ModuleCapability {
                module: module.clone(),
                proven: ProvenCapability {
                    capability,
                    evidence: Evidence::of(fact, site)
                }
            });
        }
    }
    rows.sort_by(|left, right| {
        left.module
            .cmp(&right.module)
            .then(left.proven.capability.cmp(&right.proven.capability))
    });
    rows
}

/// Derives the capabilities the program actually starts: those reachable
/// through the call graph from an entry point.
///
/// See [`Started`] for what a reachable set states and what it admits it
/// cannot state.
#[must_use]
pub fn started(workspace: &Workspace<'_>) -> Vec<Started> {
    reach::started(workspace)
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

    fn sources(text: &str) -> Vec<Source> {
        vec![Source::new(
            PathBuf::from("app/src/lib.rs"),
            text.to_owned()
        )]
    }

    #[test]
    fn a_module_binding_a_socket_serves_and_says_what_proved_it() {
        let sources =
            sources("pub fn run() { std::net::TcpListener::bind(\"0.0.0.0:80\").ok(); }");
        let rows = capabilities(&Workspace::resolved(&sources, &crate::semantic::settling::compiler_over(&sources)));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].proven.capability, Capability::Serves);
        assert_eq!(rows[0].proven.evidence.function, "run");
        assert_eq!(rows[0].proven.evidence.call, "std::net::TcpListener::bind");
        assert!(rows[0].proven.evidence.range.end > rows[0].proven.evidence.range.start);
    }

    #[test]
    fn one_module_can_hold_several_capabilities() {
        let sources = sources(
            "pub fn run() { std::net::TcpListener::bind(\"0\").ok(); }\npub fn conf() { \
             std::env::var(\"PORT\").ok(); }"
        );
        let rows = capabilities(&Workspace::resolved(&sources, &crate::semantic::settling::compiler_over(&sources)));
        let found: Vec<Capability> = rows.iter().map(|row| row.proven.capability).collect();
        assert_eq!(found, vec![Capability::Serves, Capability::Configures]);
    }

    #[test]
    fn a_capability_proved_twice_is_stated_once() {
        let sources = sources(
            "pub fn a() { std::env::var(\"A\").ok(); }\npub fn b() { std::env::var(\"B\").ok(); }"
        );
        let rows = capabilities(&Workspace::resolved(&sources, &crate::semantic::settling::compiler_over(&sources)));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].proven.evidence.function, "a");
    }

    #[test]
    fn a_module_touching_nothing_outside_gets_no_row() {
        let sources = sources("pub fn add(left: u8, right: u8) -> u8 { left + right }");
        assert!(capabilities(&Workspace::resolved(&sources, &crate::semantic::settling::compiler_over(&sources))).is_empty());
    }

    #[test]
    fn unsafe_alone_proves_no_capability() {
        let sources = sources("pub fn poke() { unsafe { } }");
        assert!(capabilities(&Workspace::resolved(&sources, &crate::semantic::settling::compiler_over(&sources))).is_empty());
    }
}
