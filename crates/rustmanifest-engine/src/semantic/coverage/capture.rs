// SPDX-FileCopyrightText: 2026 RAprogramm <andrey.rozanov.vl@gmail.com>
// SPDX-License-Identifier: MIT

//! What the type graph captured from an analyzed source slice.
//!
//! Every counter here is read straight off the graph the semantic tier
//! builds: the definitions it recorded, the members it recorded under
//! them, and the edges it could resolve between them. Nothing is inferred
//! on top — a definition the graph did not record is simply absent from
//! the counts, which is exactly the gap the measurement is meant to show.

use super::report::count;
use crate::semantic::graph::{TypeGraph, TypeKind};

/// Counters of the definitions and edges the type graph recorded.
pub(super) struct Capture {
    /// Structure definitions captured by the type graph.
    pub(super) structs:           u64,
    /// Enum definitions captured by the type graph.
    pub(super) enums:             u64,
    /// Trait definitions captured by the type graph.
    pub(super) traits:            u64,
    /// Impl blocks captured by the type graph.
    pub(super) impls:             u64,
    /// Methods and associated functions captured across all impl blocks.
    pub(super) methods:           u64,
    /// Named fields captured across all structures and enum payloads.
    pub(super) fields:            u64,
    /// Fields whose type name resolved to a workspace type.
    pub(super) aggregation_edges: u64,
    /// Type-to-type interaction edges resolved from method bodies.
    pub(super) interaction_edges: u64
}

/// Counts what the type graph built from `sources` captured.
pub(super) fn capture(types: &TypeGraph) -> Capture {
    let fields = types
        .types()
        .iter()
        .map(|node| count(node.fields.len()))
        .sum();
    let structs = types
        .types()
        .iter()
        .filter(|node| node.kind == TypeKind::Struct)
        .count();
    let enums = types.types().len() - structs;
    let methods = types
        .impls()
        .iter()
        .map(|node| count(node.methods.len()))
        .sum();
    Capture {
        structs: count(structs),
        enums: count(enums),
        traits: count(types.traits().len()),
        impls: count(types.impls().len()),
        methods,
        fields,
        aggregation_edges: count(types.aggregation_edges().len()),
        interaction_edges: count(types.interaction_edges().len())
    }
}
