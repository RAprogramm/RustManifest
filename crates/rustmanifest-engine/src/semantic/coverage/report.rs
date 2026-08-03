// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The counter record a measurement produces and the rates read off it.
//!
//! Every counter is an absolute count of something observed; a rate is a
//! division of two of them and adds no knowledge of its own. Keeping the
//! record and its arithmetic here leaves the measurement modules free to
//! report counts and nothing else.

/// Raw coverage counters of one analyzed source slice.
///
/// Every field is an absolute count; percentages are computed by the caller
/// via [`percent`] or the convenience rate methods. `files_total` counts the
/// sources handed to [`coverage`](super::coverage), so files skipped before
/// that point — unreadable or over the memory budget — are not represented
/// here.
///
/// `derives_seen` and `item_macros_seen` are sightings, not understanding:
/// macro and derive expansions are not analyzed, so anything they generate
/// is absent from the module, type, and `use` counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Coverage {
    /// Sources in the analyzed slice.
    pub files_total:                u64,
    /// Sources accepted by the Rust parser.
    pub files_parsed:               u64,
    /// Nodes in the module graph, including single-file fallback roots and
    /// inline modules.
    pub modules_total:              u64,
    /// Distinct files that became at least one module reachable from a
    /// cargo-layout crate root (`src/lib.rs`, `src/main.rs`, `src/bin/*`,
    /// `build.rs`, top-level `tests/`, `benches/`, `examples/`).
    pub files_reachable:            u64,
    /// Distinct files claimed by no layout root and promoted to standalone
    /// crate roots of their own (compile-test fixtures, broken `mod`
    /// chains, lone reviewed files).
    pub files_standalone:           u64,
    /// Structure definitions captured by the type graph.
    pub structs_captured:           u64,
    /// Enum definitions captured by the type graph.
    pub enums_captured:             u64,
    /// Trait definitions captured by the type graph.
    pub traits_captured:            u64,
    /// Impl blocks captured by the type graph.
    pub impls_captured:             u64,
    /// Methods and associated functions captured across all impl blocks.
    pub methods_captured:           u64,
    /// Named fields captured across all structures and enum payloads.
    pub fields_captured:            u64,
    /// Fields whose type name resolved to a workspace type.
    pub aggregation_edges_resolved: u64,
    /// Type-to-type interaction edges resolved from method bodies.
    pub interaction_edges_resolved: u64,
    /// Expanded `use` targets across all modules.
    pub use_targets_total:          u64,
    /// `use` targets that resolve to a module of the same crate.
    pub use_targets_resolved:       u64,
    /// `use` targets whose leading segment names a known external crate:
    /// the standard-library family or a declared dependency.
    pub use_targets_external:       u64,
    /// `use` targets that neither resolve in-workspace nor name a known
    /// external crate — the honest "did not understand" bucket.
    pub use_targets_unresolved:     u64,
    /// `#[derive(...)]` attributes seen on items; their expansions are not
    /// analyzed.
    pub derives_seen:               u64,
    /// Item-level macro invocations seen; their expansions are not
    /// analyzed.
    pub item_macros_seen:           u64,
    /// Call candidates observed across all function bodies.
    pub call_candidates_total:      u64,
    /// Call candidates that resolved to exactly one workspace function.
    pub call_candidates_resolved:   u64,
    /// Unmatched call candidates provably outside the workspace: the
    /// owner or called name matches nothing the workspace defines.
    pub call_candidates_external:   u64,
    /// Unmatched call candidates a typed receiver would settle
    /// in-workspace — the engine's own understanding gap.
    pub call_candidates_unknown:    u64,
    /// Call edges added by a SCIP index overlay beyond the syntactic
    /// candidates; zero when no index was supplied.
    pub call_candidates_scip:       u64
}

impl Coverage {
    /// Share of files the parser accepted, in percent of `files_total`.
    #[must_use]
    pub const fn parse_rate(&self) -> f64 {
        percent(self.files_parsed, self.files_total)
    }

    /// Share of `use` targets resolved in-workspace, in percent of
    /// `use_targets_total`.
    #[must_use]
    pub const fn resolution_rate(&self) -> f64 {
        percent(self.use_targets_resolved, self.use_targets_total)
    }

    /// Share of files reachable from a crate root, in percent of
    /// `files_total`.
    #[must_use]
    pub const fn reachability_rate(&self) -> f64 {
        percent(self.files_reachable, self.files_total)
    }

    /// Share of `use` targets accounted for — resolved in-workspace or
    /// attributed to a known external crate — in percent of
    /// `use_targets_total`.
    #[must_use]
    pub const fn accounted_rate(&self) -> f64 {
        percent(
            self.use_targets_resolved + self.use_targets_external,
            self.use_targets_total
        )
    }

    /// Share of call candidates that resolved to exactly one workspace
    /// function, in percent of `call_candidates_total`.
    #[must_use]
    pub const fn call_resolution_rate(&self) -> f64 {
        percent(self.call_candidates_resolved, self.call_candidates_total)
    }

    /// Share of call candidates accounted for — resolved in-workspace or
    /// attributed to external code — in percent of
    /// `call_candidates_total`.
    #[must_use]
    pub const fn call_accounted_rate(&self) -> f64 {
        percent(
            self.call_candidates_resolved + self.call_candidates_external,
            self.call_candidates_total
        )
    }
}

/// Returns `part` as a percentage of `whole`, or `0.0` when `whole` is zero.
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    reason = "coverage counters stay far below 2^52, so the casts are exact"
)]
pub const fn percent(part: u64, whole: u64) -> f64 {
    if whole == 0 {
        return 0.0;
    }
    part as f64 / whole as f64 * 100.0
}

/// Converts a collection length into a saturating `u64` counter.
pub(super) fn count(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code is allowed to fail loudly on broken invariants"
)]
mod tests {
    use super::percent;

    fn close(left: f64, right: f64) -> bool {
        (left - right).abs() < 1e-9
    }

    #[test]
    fn percent_handles_zero_whole() {
        assert!(close(percent(1, 2), 50.0));
        assert!(close(percent(0, 0), 0.0));
        assert!(close(percent(3, 0), 0.0));
    }
}
