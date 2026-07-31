// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! `structure-size` semantic check: an oversized structure is reported
//! together with a concrete split suggestion derived from the type graph
//! (`STRUCTURE.md` §3).
//!
//! Instead of only stating the field count, the check reasons over
//! relationships: field groups the structure shares with other structures
//! become "extract a common component" suggestions, fields whose type is
//! itself a workspace structure are acknowledged as components already, and
//! the remainder is listed for grouping by responsibility.

use super::{Insight, TypeGraph, field_group, graph::TypeNode};

/// Maximum number of named fields a structure may declare.
const FIELD_LIMIT: usize = 4;

/// Minimum size of a shared field group worth extracting.
const GROUP_SIZE: usize = 2;

/// Reports structures over the field limit with a graph-derived split plan.
pub(super) fn insights(graph: &TypeGraph) -> Vec<Insight> {
    let mut found = Vec::new();
    for node in graph.types() {
        let count = node.fields.len();
        if count <= FIELD_LIMIT {
            continue;
        }
        found.push(Insight::new(
            node.source,
            node.range,
            format!(
                "struct `{}` declares {count} fields (limit {FIELD_LIMIT}); {}",
                node.name,
                split_plan(graph, node)
            ),
        ));
    }
    found
}

/// Builds the human-readable split suggestion for one oversized structure.
fn split_plan(graph: &TypeGraph, node: &TypeNode) -> String {
    let mut remaining: Vec<&str> = node
        .fields
        .iter()
        .map(|field| field.name.as_str())
        .collect();
    let mut sentences = Vec::new();

    for (group, owners) in shared_groups(graph, node) {
        remaining.retain(|name| !group.contains(&(*name).to_owned()));
        sentences.push(format!(
            "extract {{{}}} into a shared component (also declared by {})",
            group.join(", "),
            owners.join(", ")
        ));
    }

    let components: Vec<String> = node
        .fields
        .iter()
        .filter(|field| remaining.contains(&field.name.as_str()))
        .filter(|field| graph.type_named(&field.type_name).is_some())
        .map(|field| format!("`{}` ({})", field.name, field.type_name))
        .collect();
    if !components.is_empty() {
        remaining.retain(|name| {
            node.fields
                .iter()
                .find(|field| field.name == *name)
                .is_none_or(|field| graph.type_named(&field.type_name).is_none())
        });
        sentences.push(format!(
            "{} already aggregate workspace types and can stay",
            components.join(", ")
        ));
    }

    if !remaining.is_empty() {
        sentences.push(format!(
            "group the rest {{{}}} by responsibility",
            remaining.join(", ")
        ));
    }

    if sentences.is_empty() {
        "split responsibilities through composition".to_owned()
    } else {
        format!("split suggestion: {}", sentences.join("; "))
    }
}

/// Returns disjoint shared field groups for `node`, largest first, each with
/// the sorted list of other structures declaring the full group.
fn shared_groups(graph: &TypeGraph, node: &TypeNode) -> Vec<(Vec<String>, Vec<String>)> {
    let mut candidates: Vec<Vec<String>> = graph
        .types()
        .iter()
        .filter(|other| other.name != node.name)
        .map(|other| field_group::shared_fields(node, other))
        .filter(|group| group.len() >= GROUP_SIZE)
        .collect();
    candidates.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
    candidates.dedup();

    let mut consumed: Vec<String> = Vec::new();
    let mut chosen = Vec::new();
    for group in candidates {
        if group.iter().any(|name| consumed.contains(name)) {
            continue;
        }
        let owners: Vec<String> = graph
            .types()
            .iter()
            .filter(|other| other.name != node.name)
            .filter(|other| field_group::owns_group(other, &group))
            .map(|other| format!("`{}`", other.name))
            .collect();
        if owners.is_empty() {
            continue;
        }
        consumed.extend(group.iter().cloned());
        chosen.push((group, owners));
    }
    chosen
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

    use super::insights;
    use crate::{semantic::TypeGraph, source::Source};

    fn graph(text: &str) -> TypeGraph {
        let source = Source::new(PathBuf::from("src/lib.rs"), text.to_owned());
        TypeGraph::from_sources(std::slice::from_ref(&source))
    }

    #[test]
    fn small_structs_are_not_reported() {
        let graph = graph("struct S { a: u8, b: u8, c: u8, d: u8 }");
        assert!(insights(&graph).is_empty());
    }

    #[test]
    fn shared_group_becomes_extraction_advice() {
        let graph = graph(
            "struct Big { id: u64, created_at: u64, a: u8, b: u8, c: u8 }\nstruct Other { id: \
             u64, created_at: u64 }",
        );
        let found = insights(&graph);
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn aggregated_workspace_types_are_acknowledged() {
        let graph =
            graph("struct Part { x: u8 }\nstruct Big { part: Part, a: u8, b: u8, c: u8, d: u8 }");
        assert_eq!(insights(&graph).len(), 1);
    }

    #[test]
    fn plain_oversize_still_reports_with_grouping_advice() {
        let graph = graph("struct Big { a: u8, b: u8, c: u8, d: u8, e: u8 }");
        assert_eq!(insights(&graph).len(), 1);
    }

    #[test]
    fn detection_is_deterministic() {
        let text = "struct Big { id: u64, created_at: u64, a: u8, b: u8, c: u8 }\nstruct Other { \
                    id: u64, created_at: u64 }";
        let first: Vec<String> = insights(&graph(text))
            .into_iter()
            .map(|insight| format!("{insight:?}"))
            .collect();
        let second: Vec<String> = insights(&graph(text))
            .into_iter()
            .map(|insight| format!("{insight:?}"))
            .collect();
        assert_eq!(first, second);
    }
}
