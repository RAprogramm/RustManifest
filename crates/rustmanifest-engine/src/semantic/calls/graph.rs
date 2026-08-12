// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The resolved call graph itself: the edge lists, the resolution
//! tallies, and the provenance of every edge.

use std::collections::BTreeSet;

/// Resolved workspace call graph over one behavioral fact slice.
#[derive(Debug)]
pub struct CallGraph {
    /// Callee fact indices per caller fact index.
    pub(super) edges:             Vec<Vec<usize>>,
    /// Call candidates that resolved to exactly one known function.
    pub(super) resolved:          u64,
    /// Candidates that were ambiguous or named no known function.
    pub(super) unresolved:        u64,
    /// Unmatched candidates provably outside the workspace: the owner
    /// names no workspace type or module, or no workspace function
    /// carries the called name at all — external by elimination.
    pub(super) external:          u64,
    /// Unmatched `?::method` candidates whose method name the workspace
    /// does define — a typed receiver would settle them, so they are the
    /// engine's own understanding gap.
    pub(super) unknown_receiver:  u64,
    /// Candidates whose name the workspace defines, with no definition
    /// the caller's build target can link against. For the caller such a
    /// function does not exist, so the call is external by elimination —
    /// but it is a different unknowing than a name the workspace never
    /// declares, and mixing the two hid one behind the other.
    pub(super) unreachable_named: u64,
    /// Candidates matching more than one linkable workspace function
    /// with no dispatch ground to carry all of them: which one is meant
    /// is not provable, and the uniqueness gate refuses rather than
    /// picks.
    pub(super) ambiguous:         u64,
    /// Qualified candidates whose owner the workspace declares but whose
    /// name that owner does not carry anywhere the tables see — the
    /// signature of a generated or trait-provided method the syntactic
    /// slice has no body for.
    pub(super) owned_unmatched:   u64,
    /// Edges added by the SCIP overlay that the syntactic candidates
    /// alone had not produced — resolution owed to the index, kept as
    /// pairs so every consumer can state an edge's provenance.
    pub(super) indexed:           BTreeSet<(usize, usize)>,
    /// Edges settled among several syntactic name matches by the index:
    /// the candidates proposed the set, the index confirmed exactly one
    /// member, and the uniqueness gate bound that one — certainty owed
    /// to both layers, kept as pairs for the same provenance duty.
    pub(super) settled:           BTreeSet<(usize, usize)>,
    /// Edges born from dispatch over several implementors — the callee
    /// is one possibility among siblings, not a certainty.
    pub(super) possibilistic:     BTreeSet<(usize, usize)>
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

    /// Candidates named in the workspace but linkable from nowhere the
    /// caller builds — external by elimination, on record as their own
    /// unknowing.
    #[must_use]
    pub const fn unreachable_named(&self) -> u64 {
        self.unreachable_named
    }

    /// Candidates matching several linkable functions, refused by the
    /// uniqueness gate rather than guessed.
    #[must_use]
    pub const fn ambiguous(&self) -> u64 {
        self.ambiguous
    }

    /// Candidates whose declared owner carries no such name in the
    /// syntactic slice.
    #[must_use]
    pub const fn owned_unmatched(&self) -> u64 {
        self.owned_unmatched
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

    /// Candidates the index settled among several syntactic matches —
    /// refusals the ambiguous row would otherwise hold.
    #[must_use]
    pub fn scip_settled(&self) -> u64 {
        u64::try_from(self.settled.len()).unwrap_or(u64::MAX)
    }

    /// Returns whether the edge was settled among several name matches
    /// by the index — the syntax proposed the set, the index chose.
    #[must_use]
    pub fn settled(&self, from: usize, to: usize) -> bool {
        self.settled.contains(&(from, to))
    }

    /// Returns whether the edge came from dispatch over several
    /// implementors — a possibility, not a certainty.
    #[must_use]
    pub fn possibilistic(&self, from: usize, to: usize) -> bool {
        self.possibilistic.contains(&(from, to))
    }
}
