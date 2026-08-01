// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! Memory-shape facts: what each workspace type weighs and where its
//! bytes live, derived from definitions and guaranteed layout rules.
//!
//! Sizes are stated for 64-bit targets and only where the language or
//! std documentation guarantees them: primitives, references, boxes,
//! the vector/string headers. The compiler is free to shuffle default
//! `repr(Rust)` layouts, so a struct's exact size is never claimed;
//! instead every type carries its `payload` — the summed guaranteed
//! sizes of its fields, the bytes a copy provably touches — and the
//! count of heap indirections its fields own. Field types the engine
//! cannot size are listed as unresolved, never estimated.

use std::collections::BTreeMap;

use super::graph::{TypeGraph, TypeKind};
use crate::source::Source;

/// Byte size of one field type on a 64-bit target, when guaranteed.
fn guaranteed_size(type_name: &str) -> Option<u64> {
    match type_name {
        "bool" | "i8" | "u8" => Some(1),
        "i16" | "u16" => Some(2),
        "char" | "f32" | "i32" | "u32" => Some(4),
        "Arc" | "Box" | "NonZeroU32" | "NonZeroU64" | "NonZeroUsize" | "Rc" | "f64" | "i64"
        | "isize" | "u64" | "usize" => Some(8),
        "Duration" | "i128" | "u128" => Some(16),
        "OsString" | "PathBuf" | "String" | "Vec" => Some(24),
        _ => None
    }
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
pub fn shapes(sources: &[Source]) -> Vec<TypeShape> {
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
                guaranteed_size(&field.type_name)
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
                    guaranteed_size(&field.type_name).is_none()
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
                copyable: node.derives.iter().any(|derive| derive == "Copy"),
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

    use super::shapes;
    use crate::source::Source;

    fn shapes_of(text: &str) -> Vec<super::TypeShape> {
        shapes(&[Source::new(PathBuf::from("src/lib.rs"), text.to_owned())])
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

    #[test]
    fn copy_derives_are_recorded() {
        let all = shapes_of("#[derive(Clone, Copy)]\nstruct Id { value: u64 }");
        assert!(named(&all, "Id").copyable);
        let plain = shapes_of("struct Id { value: u64 }");
        assert!(!named(&plain, "Id").copyable);
    }
}
