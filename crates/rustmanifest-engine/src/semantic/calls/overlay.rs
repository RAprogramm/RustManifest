// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Application of an externally bound edge set — the SCIP overlay — onto
//! the syntactically resolved edges, with the provenance record of what
//! the index alone contributed.

use std::collections::BTreeSet;

/// Applies overlay callee indices onto the edge lists and returns the
/// edges the overlay added beyond the ones already present, as
/// `(caller, callee)` pairs — the provenance record a consumer reads to
/// tell an index-resolved edge from a syntactic one.
///
/// `admits` holds the same build-target constraint the syntactic
/// resolution obeys, so the index cannot introduce an edge across a
/// target boundary either.
pub(super) fn apply_overlay(
    edges: &mut [Vec<usize>],
    overlay: &[Vec<usize>],
    admits: &dyn Fn(usize, usize) -> bool
) -> BTreeSet<(usize, usize)> {
    let mut added = BTreeSet::new();
    for (index, callees) in overlay.iter().enumerate().take(edges.len()) {
        for &callee in callees {
            if !admits(index, callee) {
                continue;
            }
            if !edges[index].contains(&callee) {
                edges[index].push(callee);
                added.insert((index, callee));
            }
        }
        edges[index].sort_unstable();
    }
    added
}
