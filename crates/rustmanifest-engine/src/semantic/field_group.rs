// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! `repeated-field-group` check: an identical group of fields appearing in
//! two or more structures is a shared concept waiting to be extracted into
//! its own component (`STRUCTURE.md` §3, composition).

use std::collections::BTreeMap;

use super::{Insight, TypeGraph, graph::TypeNode};

/// Minimum number of shared fields that makes a group worth extracting.
const GROUP_SIZE: usize = 2;

/// Reports maximal field groups (name and type identical) shared by two or
/// more structures, anchored at the first structure declaring the group.
pub(super) fn insights(graph: &TypeGraph) -> Vec<Insight> {
    let mut found = Vec::new();
    let mut reported: Vec<Vec<String>> = Vec::new();
    let types = graph.types();
    for (position, first) in types.iter().enumerate() {
        for second in &types[position + 1..] {
            let shared = shared_fields(first, second);
            if shared.len() < GROUP_SIZE || reported.contains(&shared) {
                continue;
            }
            let owners: Vec<&str> = types
                .iter()
                .filter(|node| owns_group(node, &shared))
                .map(|node| node.name.as_str())
                .collect();
            reported.push(shared.clone());
            found.push(Insight::new(
                first.source,
                first.range,
                format!(
                    "structures {} all declare the field group {{{}}}; the group is a shared \
                     concept — extract it into its own component and aggregate it",
                    owners
                        .iter()
                        .map(|name| format!("`{name}`"))
                        .collect::<Vec<_>>()
                        .join(", "),
                    shared.join(", ")
                )
            ));
        }
    }
    found
}

/// Returns the sorted list of field names two structures share with equal
/// name and type.
fn shared_fields(first: &TypeNode, second: &TypeNode) -> Vec<String> {
    let mut by_name: BTreeMap<&str, &str> = BTreeMap::new();
    for field in &first.fields {
        by_name.insert(&field.name, &field.type_name);
    }
    let mut shared: Vec<String> = second
        .fields
        .iter()
        .filter(|field| by_name.get(field.name.as_str()) == Some(&field.type_name.as_str()))
        .map(|field| field.name.clone())
        .collect();
    shared.sort();
    shared
}

/// Returns true when `node` declares every field of `group` by name.
fn owns_group(node: &TypeNode, group: &[String]) -> bool {
    group
        .iter()
        .all(|name| node.fields.iter().any(|field| &field.name == name))
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
    fn repeated_pair_is_reported_once() {
        let graph = graph(
            "struct User { id: u64, created_at: u64, updated_at: u64 }\nstruct Post { body: \
             String, created_at: u64, updated_at: u64 }\nstruct Tag { label: String, created_at: \
             u64, updated_at: u64 }"
        );
        let insights = insights(&graph);
        assert_eq!(insights.len(), 1);
    }

    #[test]
    fn single_shared_field_is_not_a_group() {
        let graph = graph("struct A { id: u64, x: u8 }\nstruct B { id: u64, y: u8 }");
        assert!(insights(&graph).is_empty());
    }

    #[test]
    fn same_name_different_type_does_not_match() {
        let graph = graph(
            "struct A { created_at: u64, updated_at: u64 }\nstruct B { created_at: String, \
             updated_at: u64 }"
        );
        assert!(insights(&graph).is_empty());
    }

    #[test]
    fn disjoint_structures_pass() {
        let graph = graph("struct A { a: u8, b: u8 }\nstruct B { c: u8, d: u8 }");
        assert!(insights(&graph).is_empty());
    }
}
