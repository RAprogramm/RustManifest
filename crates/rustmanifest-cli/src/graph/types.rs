// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Rendering of the type diagram from a type graph.

use std::collections::BTreeSet;

use rustmanifest_engine::TypeGraph;

use super::options::DiagramFormat;

/// Renders the type diagram, one output line per element: solid arrows for
/// aggregation (labeled with the field), dashed arrows for interaction (a
/// method body of the source type calling into the target type).
///
/// Nodes are sorted by type name and deduplicated; edges are sorted by
/// `(owner, label, target)`.
pub(super) fn type_lines(graph: &TypeGraph, format: DiagramFormat) -> Vec<String> {
    let nodes: BTreeSet<&str> = graph
        .types()
        .iter()
        .map(|node| node.name.as_str())
        .collect();
    let edges: BTreeSet<(&str, &str, &str)> = graph
        .aggregation_edges()
        .into_iter()
        .map(|(owner, field, target)| {
            (
                owner.name.as_str(),
                field.name.as_str(),
                target.name.as_str(),
            )
        })
        .collect();
    let interactions: BTreeSet<(&str, &str)> = graph
        .interaction_edges()
        .into_iter()
        .map(|(impl_block, target)| (impl_block.type_name.as_str(), target.name.as_str()))
        .filter(|(from, to)| {
            !edges
                .iter()
                .any(|(owner, _, target)| owner == from && target == to)
        })
        .collect();
    match format {
        DiagramFormat::Mermaid => {
            let mut lines = vec!["graph LR".to_owned()];
            for name in &nodes {
                lines.push(format!("    {name}[\"{name}\"]"));
            }
            for (owner, field, target) in &edges {
                lines.push(format!("    {owner} -->|{field}| {target}"));
            }
            for (from, to) in &interactions {
                lines.push(format!("    {from} -.->|calls| {to}"));
            }
            lines
        }
        DiagramFormat::Dot => {
            let mut lines = vec!["digraph types {".to_owned(), "    rankdir=LR;".to_owned()];
            for name in &nodes {
                lines.push(format!("    \"{name}\";"));
            }
            for (owner, field, target) in &edges {
                lines.push(format!(
                    "    \"{owner}\" -> \"{target}\" [label=\"{field}\"];"
                ));
            }
            for (from, to) in &interactions {
                lines.push(format!(
                    "    \"{from}\" -> \"{to}\" [style=\"dashed\" label=\"calls\"];"
                ));
            }
            lines.push("}".to_owned());
            lines
        }
    }
}
