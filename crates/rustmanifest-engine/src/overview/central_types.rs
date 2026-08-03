// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Central types: the types the rest of the code leans on most, ranked by
//! the aggregation and interaction edges resolved around them.

use super::fragment::{push, truncation_note};
use crate::semantic::TypeGraph;

/// Prints the most connected types.
pub(super) fn type_section(out: &mut String, graph: &TypeGraph, top: usize) {
    out.push_str("## Central types\n\n");
    let mut connectivity: Vec<(String, usize)> = graph
        .types()
        .iter()
        .map(|node| {
            let inbound = graph.aggregators_of(&node.name).len();
            let outbound = graph
                .aggregation_edges()
                .iter()
                .filter(|(owner, _, _)| owner.name == node.name)
                .count();
            let interactions = graph
                .interaction_edges()
                .iter()
                .filter(|(_, target)| target.name == node.name)
                .count();
            (node.name.clone(), inbound + outbound + interactions)
        })
        .filter(|(_, connections)| *connections > 0)
        .collect();
    connectivity.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    if connectivity.is_empty() {
        out.push_str("- no aggregation or interaction edges resolved\n\n");
        return;
    }
    for (name, connections) in connectivity.iter().take(top) {
        push(out, &format!("- `{name}`: {connections} connections\n"));
    }
    truncation_note(out, connectivity.len(), top);
    out.push('\n');
}
