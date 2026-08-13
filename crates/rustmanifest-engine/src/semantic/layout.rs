// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Memory-shape facts: what each workspace type weighs and where its
//! bytes live.
//!
//! A field's size is the compiler's answer for the target it was asked
//! about, carried by a resolution. The table that stood here stated sizes
//! for 64-bit targets and said nothing about being wrong on any other — a
//! pointer is eight bytes until the target is a thirty-two-bit one.
//!
//! The compiler is free to shuffle default `repr(Rust)` layouts, so a
//! struct's exact size is never claimed here; every type carries its
//! `payload` — the summed sizes of its fields, the bytes a copy provably
//! touches — and the count of heap indirections its fields own. A field
//! type no resolution sized is listed as unresolved, never estimated.

use std::collections::BTreeMap;

use super::graph::{TypeGraph, TypeKind};
use crate::source::Source;

/// Byte size of one field type, as the compiler laid it out.
fn sized(settled: Option<&rustmanifest_schema::Resolution>, type_name: &str) -> Option<u64> {
    Some(settled?.weighs(type_name)?.size)
}

/// Whether one field type owns heap storage behind its header.
fn owns_heap(type_name: &str) -> bool {
    matches!(
        type_name,
        "Arc"
            | "BTreeMap"
            | "BTreeSet"
            | "Box"
            | "HashMap"
            | "HashSet"
            | "OsString"
            | "PathBuf"
            | "Rc"
            | "String"
            | "Vec"
            | "VecDeque"
    )
}

/// Memory shape of one workspace type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeShape {
    /// Type name.
    pub name:         String,
    /// Summed guaranteed byte sizes of the fields — the bytes a copy
    /// provably touches; `None` for enums, whose live variant is a
    /// runtime property.
    pub payload:      Option<u64>,
    /// Fields owning heap storage behind their stack header.
    pub indirections: u32,
    /// Whether the definition derives `Copy`.
    pub copyable:     bool,
    /// Field type names the engine could not size, honestly listed.
    pub unresolved:   Vec<String>
}

/// Computes the memory shape of every type the graph captured.
///
/// Workspace types nest: a field typed by another workspace struct
/// contributes that struct's payload, resolved to a fixpoint with
/// cycles broken as unresolved.
#[must_use]
pub fn shapes_with(
    sources: &[Source],
    settled: Option<&rustmanifest_schema::Resolution>
) -> Vec<TypeShape> {
    let graph = TypeGraph::from_sources(sources);
    let mut payloads: BTreeMap<&str, Option<u64>> = BTreeMap::new();
    for node in graph.types() {
        if node.kind == TypeKind::Struct {
            payloads.insert(&node.name, None);
        }
    }
    for _ in 0..graph.types().len() {
        let mut changed = false;
        for node in graph.types() {
            if node.kind != TypeKind::Struct || payloads.get(node.name.as_str()) != Some(&None) {
                continue;
            }
            let summed: Option<u64> = node.fields.iter().try_fold(0u64, |total, field| {
                sized(settled, &field.type_name)
                    .or_else(|| payloads.get(field.type_name.as_str()).copied().flatten())
                    .map(|size| total + size)
            });
            if summed.is_some() {
                payloads.insert(&node.name, summed);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    graph
        .types()
        .iter()
        .map(|node| {
            let unresolved: Vec<String> = node
                .fields
                .iter()
                .filter(|field| {
                    sized(settled, &field.type_name).is_none()
                        && payloads
                            .get(field.type_name.as_str())
                            .copied()
                            .flatten()
                            .is_none()
                })
                .map(|field| field.type_name.clone())
                .collect();
            TypeShape {
                name: node.name.clone(),
                payload: if node.kind == TypeKind::Struct {
                    payloads.get(node.name.as_str()).copied().flatten()
                } else {
                    None
                },
                indirections: u32::try_from(
                    node.fields
                        .iter()
                        .filter(|field| owns_heap(&field.type_name))
                        .count()
                )
                .unwrap_or(u32::MAX),
                copyable: graph.ability_given(&node.name, "Copy"),
                unresolved
            }
        })
        .collect()
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

    use super::shapes_with;
    use crate::source::Source;

    /// The shapes of `text` with the sizes a compiler would state for the
    /// types the fixture writes.
    fn shapes_of(text: &str) -> Vec<super::TypeShape> {
        let mut settled = rustmanifest_schema::Resolution {
            resolver: "test".to_owned(),
            layouts: [
                ("bool", 1u64),
                ("u8", 1),
                ("i8", 1),
                ("u16", 2),
                ("i16", 2),
                ("char", 4),
                ("f32", 4),
                ("i32", 4),
                ("u32", 4),
                ("f64", 8),
                ("i64", 8),
                ("u64", 8),
                ("usize", 8),
                ("isize", 8),
                ("Box", 8),
                ("Arc", 8),
                ("Rc", 8),
                ("i128", 16),
                ("u128", 16),
                ("Duration", 16),
                ("String", 24),
                ("Vec", 24),
                ("PathBuf", 24),
                ("OsString", 24)
            ]
            .into_iter()
            .map(|(named, size)| rustmanifest_schema::ResolvedLayout {
                named: named.to_owned(),
                size,
                align: size.min(8)
            })
            .collect(),
            ..rustmanifest_schema::Resolution::default()
        };
        settled.settle();
        shapes_with(
            &[Source::new(PathBuf::from("src/lib.rs"), text.to_owned())],
            Some(&settled)
        )
    }

    fn named<'a>(all: &'a [super::TypeShape], name: &str) -> &'a super::TypeShape {
        all.iter().find(|shape| shape.name == name).unwrap()
    }

    #[test]
    fn primitive_fields_sum_into_the_payload() {
        let all = shapes_of("struct Point { x: f64, y: f64, tag: u8 }");
        let point = named(&all, "Point");
        assert_eq!(point.payload, Some(17));
        assert_eq!(point.indirections, 0);
        assert!(point.unresolved.is_empty());
    }

    #[test]
    fn heap_headers_count_and_weigh() {
        let all = shapes_of("struct Doc { title: String, body: Vec<u8>, id: u64 }");
        let doc = named(&all, "Doc");
        assert_eq!(doc.payload, Some(56));
        assert_eq!(doc.indirections, 2);
    }

    #[test]
    fn nested_workspace_types_resolve_to_a_fixpoint() {
        let all = shapes_of(
            "struct Inner { a: u32, b: u32 }\nstruct Outer { inner: Inner, flag: bool }"
        );
        assert_eq!(named(&all, "Outer").payload, Some(9));
    }

    #[test]
    fn unknown_field_types_stay_honestly_unresolved() {
        let all = shapes_of("struct Holder { data: ExternalThing, count: u32 }");
        let holder = named(&all, "Holder");
        assert_eq!(holder.payload, None);
        assert_eq!(holder.unresolved, vec!["ExternalThing"]);
    }

}
