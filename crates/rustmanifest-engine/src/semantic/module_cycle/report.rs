// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! The insight one detected cycle turns into.
//!
//! The detail names only the cycle chain; every `use` edge of the cycle
//! becomes one trace step, and the anchor is the edge leaving the first
//! module of the canonical rotation.

use rustmanifest_schema::TraceStep;

use super::edge::UseEdge;
use crate::{
    semantic::{insight::Insight, module_graph::ModuleGraph},
    source::Source
};

/// Builds the insight for one canonical cycle, anchoring at the edge
/// leaving the first module. The detail names only the cycle chain; every
/// `use` edge becomes one trace step.
pub(super) fn cycle_insight(
    sources: &[Source],
    graph: &ModuleGraph,
    edges: &[Vec<UseEdge>],
    cycle: &[usize]
) -> Option<Insight> {
    let modules = graph.modules();
    let first = *cycle.first()?;
    let mut trace = Vec::with_capacity(cycle.len());
    for (offset, &from) in cycle.iter().enumerate() {
        let to = cycle[(offset + 1) % cycle.len()];
        let edge = edges.get(from)?.iter().find(|edge| edge.to == to)?;
        let use_target = modules[from].uses.get(edge.use_index)?;
        let file = sources.get(modules[from].source)?.path();
        trace.push(TraceStep {
            fact:   format!(
                "{} ({}) uses {}",
                modules[from].path,
                file.display(),
                use_target.target
            ),
            reason: format!("resolves to {}, an edge of the cycle", modules[to].path)
        });
    }
    let mut chain: Vec<&str> = cycle
        .iter()
        .map(|&module| modules[module].path.as_str())
        .collect();
    chain.push(modules[first].path.as_str());
    let detail = format!("module dependency cycle: {}", chain.join(" -> "));
    let anchor_edge = edges
        .get(first)?
        .iter()
        .find(|edge| edge.to == cycle[usize::from(cycle.len() > 1)])?;
    let range = modules[first].uses.get(anchor_edge.use_index)?.range;
    Some(Insight::new(modules[first].source, range, detail, trace))
}
