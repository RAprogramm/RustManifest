// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The resolved call graph itself: the edge lists, the resolution
//! tallies, and the provenance of every edge.

use std::collections::BTreeSet;

/// Resolved workspace call graph over one behavioral fact slice.
#[derive(Debug)]
pub struct CallGraph {
    /// Callee fact indices per caller fact index.
    pub(super) edges:            Vec<Vec<usize>>,
    /// Call candidates that resolved to exactly one known function.
    pub(super) resolved:         u64,
    /// Candidates that were ambiguous or named no known function.
    pub(super) unresolved:       u64,
    /// Unmatched candidates provably outside the workspace: the owner
    /// names no workspace type or module, or no workspace function
    /// carries the called name at all — external by elimination.
    pub(super) external:         u64,
    /// Unmatched `?::method` candidates whose method name the workspace
    /// does define — a typed receiver would settle them, so they are the
    /// engine's own understanding gap.
    pub(super) unknown_receiver: u64,
    /// Edges added by the SCIP overlay that the syntactic candidates
    /// alone had not produced — resolution owed to the index, kept as
    /// pairs so every consumer can state an edge's provenance.
    pub(super) indexed:          BTreeSet<(usize, usize)>,
    /// Edges born from dispatch over several implementors — the callee
    /// is one possibility among siblings, not a certainty.
    pub(super) possibilistic:    BTreeSet<(usize, usize)>
}

impl CallGraph {
    /// Callee fact indices of the function at `index`, deduplicated and
    /// sorted.
    #[must_use]
    pub fn callees(&self, index: usize) -> &[usize] {
        self.edges.get(index).map_or(&[], Vec::as_slice)
    }

    /// Call candidates that resolved to exactly one known function.
    #[must_use]
    pub const fn resolved(&self) -> u64 {
        self.resolved
    }

    /// Candidates that were ambiguous or named no known function.
    #[must_use]
    pub const fn unresolved(&self) -> u64 {
        self.unresolved
    }

    /// Unmatched candidates provably attributable to external code.
    #[must_use]
    pub const fn external(&self) -> u64 {
        self.external
    }

    /// Unmatched candidates a typed receiver would settle in-workspace.
    #[must_use]
    pub const fn unknown_receiver(&self) -> u64 {
        self.unknown_receiver
    }

    /// Edges the SCIP overlay added beyond the syntactic candidates.
    #[must_use]
    pub fn scip_resolved(&self) -> u64 {
        u64::try_from(self.indexed.len()).unwrap_or(u64::MAX)
    }

    /// Returns whether the edge exists only because the SCIP index
    /// resolved it — the syntactic candidates alone did not produce it.
    #[must_use]
    pub fn indexed(&self, from: usize, to: usize) -> bool {
        self.indexed.contains(&(from, to))
    }

    /// Returns whether the edge came from dispatch over several
    /// implementors — a possibility, not a certainty.
    #[must_use]
    pub fn possibilistic(&self, from: usize, to: usize) -> bool {
        self.possibilistic.contains(&(from, to))
    }
}
