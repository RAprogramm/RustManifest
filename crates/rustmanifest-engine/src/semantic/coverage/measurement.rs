// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The measurement pass and the entry points that start it.
//!
//! One pass runs the production machinery over a source slice — the
//! parser, the module graph, the type graph, and the call graph — and
//! assembles the counters each of them yields into a single record. The
//! entry points differ only in what they hand the pass: no declared
//! dependencies, a set of them, or a set of them plus a SCIP index
//! overlay.

use std::collections::BTreeSet;

use super::{
    capture, origin, reach,
    report::{Coverage, count},
    sighting, use_target
};
use crate::semantic::{
    calls::{call_graph_with, resolution_context},
    scip::ScipEdges,
    workspace::Workspace
};

/// Measures `sources` like [`coverage_with`] with no declared
/// dependencies: only the standard-library family counts as external.
#[must_use]
pub fn coverage(workspace: &Workspace<'_>) -> Coverage {
    coverage_with(workspace, &BTreeSet::new())
}

/// Measures how much of `sources` the engine's parsing, module graph, and
/// type graph actually capture.
///
/// The measurement reuses the production machinery unchanged: the module
/// graph decides reachability, the `module-cycles` resolution logic decides
/// which `use` targets count as in-workspace, and the type graph provides
/// the structure counters. `use` targets that do not resolve in-workspace
/// are attributed to a known external crate — the standard-library family
/// or a name in `external_crates` — or counted as honestly unresolved.
/// Macro and derive expansions are never analyzed; they are only counted
/// as sightings.
#[must_use]
pub fn coverage_with(workspace: &Workspace<'_>, external_crates: &BTreeSet<String>) -> Coverage {
    measure(workspace, external_crates, None)
}

/// Measures `sources` like [`coverage_with`] with a SCIP index overlay:
/// call edges the index resolves beyond the syntactic candidates are
/// counted separately, so the report states what the index added.
#[must_use]
pub fn coverage_indexed(
    workspace: &Workspace<'_>,
    external_crates: &BTreeSet<String>,
    edges: &ScipEdges
) -> Coverage {
    measure(workspace, external_crates, Some(edges))
}

/// Shared measurement behind [`coverage_with`] and [`coverage_indexed`].
fn measure(
    workspace: &Workspace<'_>,
    external_crates: &BTreeSet<String>,
    scip: Option<&ScipEdges>
) -> Coverage {
    let sources = workspace.sources();
    let seen = sighting::sightings(sources);
    let graph = workspace.modules();
    let modules = graph.modules();
    let files = reach::reach(modules);
    let uses = use_target::use_targets(modules, external_crates);
    let facts = workspace.facts();
    let overlay = scip.map(|edges| edges.candidates(facts, sources));
    let calls = call_graph_with(facts, &resolution_context(sources), overlay.as_deref());
    let captured = capture::capture(sources);
    let settled = origin::origins(facts);
    Coverage {
        files_total:                 count(sources.len()),
        files_parsed:                seen.files_parsed,
        modules_total:               count(modules.len()),
        files_reachable:             files.reachable,
        files_standalone:            files.standalone,
        structs_captured:            captured.structs,
        enums_captured:              captured.enums,
        traits_captured:             captured.traits,
        impls_captured:              captured.impls,
        methods_captured:            captured.methods,
        fields_captured:             captured.fields,
        aggregation_edges_resolved:  captured.aggregation_edges,
        interaction_edges_resolved:  captured.interaction_edges,
        use_targets_total:           uses.total,
        use_targets_resolved:        uses.resolved,
        use_targets_external:        uses.external,
        use_targets_unresolved:      uses.unresolved,
        derives_seen:                seen.derives,
        item_macros_seen:            seen.item_macros,
        call_candidates_total:       calls.resolved() + calls.unresolved(),
        call_candidates_resolved:    calls.resolved(),
        call_candidates_external:    calls.external(),
        call_candidates_unknown:     calls.unknown_receiver(),
        call_candidates_unreachable: calls.unreachable_named(),
        call_candidates_ambiguous:   calls.ambiguous(),
        call_candidates_owned:       calls.owned_unmatched(),
        call_candidates_scip:        calls.scip_resolved(),
        call_candidates_settled:     calls.scip_settled(),
        origins_total:               settled.total,
        origins_derived:             settled.derived
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use std::{collections::BTreeSet, path::PathBuf};

    use super::{super::unresolved_targets, coverage, coverage_with};
    use crate::{semantic::workspace::Workspace, source::Source};

    fn source(path: &str, text: &str) -> Source {
        Source::new(PathBuf::from(path), text.to_owned())
    }

    fn close(left: f64, right: f64) -> bool {
        (left - right).abs() < 1e-9
    }

    #[test]
    fn parse_failure_is_counted() {
        let sources = [
            source("src/lib.rs", "pub struct Ok;"),
            source("src/broken.rs", "fn broken(")
        ];
        let report = coverage(&Workspace::new(&sources));
        assert_eq!(report.files_total, 2);
        assert_eq!(report.files_parsed, 1);
        assert!(close(report.parse_rate(), 50.0));
    }

    #[test]
    fn unreachable_file_lowers_reachability() {
        let sources = [
            source("src/lib.rs", "mod util;"),
            source("src/util.rs", ""),
            source("src/orphan.rs", "")
        ];
        let report = coverage(&Workspace::new(&sources));
        assert_eq!(report.files_total, 3);
        assert_eq!(report.modules_total, 3);
        assert_eq!(report.files_reachable, 2);
        assert_eq!(report.files_standalone, 1);
        assert!(close(report.reachability_rate(), 200.0 / 3.0));
    }

    #[test]
    fn inline_modules_count_as_modules_of_one_file() {
        let sources = [source("src/lib.rs", "mod a {} mod b {}")];
        let report = coverage(&Workspace::new(&sources));
        assert_eq!(report.modules_total, 3);
        assert_eq!(report.files_reachable, 1);
    }

    #[test]
    fn lone_files_become_standalone_roots() {
        let sources = [source("rules/one.rs", ""), source("rules/two.rs", "")];
        let report = coverage(&Workspace::new(&sources));
        assert_eq!(report.modules_total, 2);
        assert_eq!(report.files_reachable, 0);
        assert_eq!(report.files_standalone, 2);
    }

    #[test]
    fn use_resolution_splits_workspace_and_external() {
        let sources = [
            source("src/lib.rs", "mod a; mod b;"),
            source("src/a.rs", "use crate::b::Item; use std::fmt;"),
            source("src/b.rs", "")
        ];
        let report = coverage(&Workspace::new(&sources));
        assert_eq!(report.use_targets_total, 2);
        assert_eq!(report.use_targets_resolved, 1);
        assert_eq!(report.use_targets_external, 1);
        assert_eq!(report.use_targets_unresolved, 0);
        assert!(close(report.resolution_rate(), 50.0));
        assert!(close(report.accounted_rate(), 100.0));
    }

    #[test]
    fn uniform_paths_to_child_modules_resolve_in_workspace() {
        let sources = [
            source("src/lib.rs", "mod handlers; pub use handlers::routes;"),
            source(
                "src/handlers.rs",
                "mod account; pub use account::account_routes; use serde::Serialize;"
            ),
            source("src/handlers/account.rs", "")
        ];
        let report = coverage(&Workspace::new(&sources));
        assert_eq!(report.use_targets_resolved, 2);
        assert_eq!(report.use_targets_unresolved, 1);
        let targets = unresolved_targets(&sources, &BTreeSet::new());
        assert_eq!(targets.len(), 1);
        assert_eq!(targets["serde::Serialize"], 1);
    }

    #[test]
    fn declared_dependencies_move_targets_from_unresolved_to_external() {
        let sources = [source(
            "src/lib.rs",
            "use serde::Serialize; use unknown_crate::Thing;"
        )];
        let bare = coverage(&Workspace::new(&sources));
        assert_eq!(bare.use_targets_external, 0);
        assert_eq!(bare.use_targets_unresolved, 2);
        let declared = BTreeSet::from(["serde".to_owned()]);
        let informed = coverage_with(&Workspace::new(&sources), &declared);
        assert_eq!(informed.use_targets_external, 1);
        assert_eq!(informed.use_targets_unresolved, 1);
        assert!(close(informed.accounted_rate(), 50.0));
    }

    #[test]
    fn type_graph_counters_are_reported() {
        let sources = [
            source("src/lib.rs", "pub struct Identity { pub id: u64 }"),
            source(
                "src/user.rs",
                "pub struct User { pub identity: Identity, pub age: u8 }"
            )
        ];
        let report = coverage(&Workspace::new(&sources));
        assert_eq!(report.structs_captured, 2);
        assert_eq!(report.fields_captured, 3);
        assert_eq!(report.aggregation_edges_resolved, 1);
    }

    #[test]
    fn derives_and_item_macros_are_counted() {
        let sources = [source(
            "src/lib.rs",
            "#[derive(Debug, Clone)]\npub struct A;\nmacro_rules! noise { () => {} }\nmod inner \
             { #[derive(Default)] pub struct B; }"
        )];
        let report = coverage(&Workspace::new(&sources));
        assert_eq!(report.derives_seen, 2);
        assert_eq!(report.item_macros_seen, 1);
    }

    #[test]
    fn empty_slice_yields_zero_rates() {
        let report = coverage(&Workspace::new(&[]));
        assert_eq!(report.files_total, 0);
        assert!(close(report.parse_rate(), 0.0));
        assert!(close(report.resolution_rate(), 0.0));
        assert!(close(report.reachability_rate(), 0.0));
    }

    #[test]
    fn measurement_is_deterministic() {
        let sources = [
            source(
                "src/lib.rs",
                "mod a; #[derive(Debug)] pub struct S { f: u8 }"
            ),
            source("src/a.rs", "use crate::S; use std::fmt;")
        ];
        assert_eq!(
            coverage(&Workspace::new(&sources)),
            coverage(&Workspace::new(&sources))
        );
    }
}
