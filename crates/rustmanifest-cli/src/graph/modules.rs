// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Rendering of the module dependency diagram from a module overview.

use std::collections::{BTreeMap, BTreeSet};

use rustmanifest_engine::ModuleOverview;

use super::options::DiagramFormat;

/// Renders the module dependency diagram, one output line per element.
///
/// Nodes are sorted by module path and merged across crates; edges are
/// sorted by `(from, to)` and deduplicated. Modules participating in a
/// dependency cycle are styled: Mermaid nodes get the `cycle` class, DOT
/// nodes are drawn red.
pub(super) fn module_lines(overview: &ModuleOverview, format: DiagramFormat) -> Vec<String> {
    let mut nodes: BTreeMap<&str, bool> = BTreeMap::new();
    for module in overview.modules() {
        let in_cycle = nodes.entry(module.path.as_str()).or_insert(false);
        *in_cycle = *in_cycle || module.in_cycle;
    }
    let edges: BTreeSet<(&str, &str)> = overview
        .edges()
        .iter()
        .filter(|edge| edge.from != edge.to)
        .map(|edge| (edge.from.as_str(), edge.to.as_str()))
        .collect();
    match format {
        DiagramFormat::Mermaid => {
            let mut lines = vec!["graph LR".to_owned()];
            if nodes.values().any(|&in_cycle| in_cycle) {
                lines.push("    classDef cycle stroke:#cc0000,stroke-width:2px".to_owned());
            }
            for (path, in_cycle) in &nodes {
                let id = mermaid_id(path);
                let class = if *in_cycle { ":::cycle" } else { "" };
                lines.push(format!("    {id}[\"{path}\"]{class}"));
            }
            for (from, to) in &edges {
                lines.push(format!("    {} --> {}", mermaid_id(from), mermaid_id(to)));
            }
            lines
        }
        DiagramFormat::Dot => {
            let mut lines = vec!["digraph modules {".to_owned(), "    rankdir=LR;".to_owned()];
            for (path, in_cycle) in &nodes {
                let attributes = if *in_cycle { " [color=\"red\"]" } else { "" };
                lines.push(format!("    \"{path}\"{attributes};"));
            }
            for (from, to) in &edges {
                lines.push(format!("    \"{from}\" -> \"{to}\";"));
            }
            lines.push("}".to_owned());
            lines
        }
    }
}

/// Sanitizes a module path into a Mermaid node identifier.
fn mermaid_id(path: &str) -> String {
    path.replace("::", "_")
}
